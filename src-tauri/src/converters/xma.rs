use std::{borrow::Cow, sync::atomic::{AtomicBool, AtomicUsize, Ordering}};

use anyhow::{Context as _, Result};
use html5ever::{local_name, ns, namespace_url, tendril::TendrilSink, QualName};
use lol_html::{html_content::{ContentType, EndTag}, DocumentContentHandlers, RewriteStrSettings};
use markup5ever_rcdom::{RcDom, Handle, NodeData};
use scraper::{Html, Selector, Element};
use url::Url;

pub async fn xma_to_markdown<'a>(url: String) -> Result<ConvertedInfo> {
    // validate the url
    let url = Url::parse(&url).context("could not parse url")?;
    if !matches!(url.host_str(), Some("xivmodarchive.com" | "www.xivmodarchive.com")) {
        anyhow::bail!("not an XMA url");
    }

    if !url.path().starts_with("/modid/") {
        anyhow::bail!("not an XMA url that points to a mod");
    }

    let client = reqwest::Client::new();
    let resp = client.get("https://www.xivmodarchive.com/anon_login")
        .send()
        .await?;
    let cookies = resp.headers()
        .get_all("Cookie")
        .iter()
        .cloned()
        .collect::<Vec<_>>();

    // download the page
    let resp = client
        .get(url)
        // FIXME
        // .header("Cookie", cookies)
        .send()
        .await
        .context("could not download xma page")?
        .text()
        .await
        .context("could not get xma page as text")?;

    // parse the html into a dom
    let dom = Html::parse_document(&resp);

    // selectors
    let title_s = Selector::parse(".jumbotron > .row h1").unwrap();
    let author_link_s = Selector::parse(".jumbotron > .row p.lead > a").unwrap();
    let info_s = Selector::parse("#info").unwrap();
    let tag_link_s = Selector::parse("a[href ^= '/search?tags=']").unwrap();

    // get the title of the mod
    let title = dom.select(&title_s).next()
        .context("missing title")?
        .text()
        .collect::<String>()
        .trim()
        .to_string();

    // get tags
    let tags = dom.select(&tag_link_s)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect::<Vec<_>>();

    // get if this mod is nsfw by getting the link inside the description,
    // then going up to the parent and searching for nsfw/nsfl
    let nsfw = dom.select(&author_link_s)
        .next()
        .context("missing author link")?;
    let nsfw = nsfw.parent_element()
        .context("author link had no parent")?
        .text()
        .collect::<String>()
        .to_lowercase();
    let nsfl = nsfw.contains("a nsfl");
    let nsfw = nsfw.contains("a nsfw");

    // get the info section
    let info = dom.select(&info_s).next()
        .context("missing info section")?;
    println!("{}", info.inner_html());

    let brs = AtomicUsize::default();
    let in_list = AtomicBool::default();
    // apply some transformations to help turn this into markdown
    let rewriters = vec![
        // <p class=lead> should be a heading
        lol_html::element!("p.lead", |el| {
            el.set_tag_name("h2")?;
            el.remove_attribute("class");

            Ok(())
        }),

        // heading contents should not end in colons
        lol_html::text!("p.lead", |t| {
            println!("p.lead: {:?}", t.as_str());
            let text = t.as_str().trim_end_matches(':').to_string();
            t.replace(
                &text,
                ContentType::Text,
            );

            Ok(())
        }),

        // unwrap divs
        lol_html::element!("div.px-2", |el| {
            el.remove_and_keep_content();
            // start a paragraph
            el.prepend("<p>", ContentType::Html);
            println!("starting p");
            if let Some(handlers) = el.end_tag_handlers() {
                handlers.push(Box::new(|end_tag: &mut EndTag| {
                    println!("ending p");
                    end_tag.before("</p>", ContentType::Html);
                    Ok(())
                }));
            }
            Ok(())
        }),

        // relative links should be fixed
        lol_html::element!("a[href]", |el| {
            // selector guarantees href
            let href = el.get_attribute("href").unwrap();
            let href = if matches!(Url::parse(&href), Err(url::ParseError::RelativeUrlWithoutBase)) {
                let root = Url::parse("https://xivmodarchive.com").unwrap();
                let joined = match href.as_bytes() {
                    // Scheme-relative URL
                    [b'/', b'/', ..] => root.join(&href),
                    // Path-absolute URL
                    b"/" => root.join("."),
                    [b'/', ..] => root.join(&href[1..]),
                    // Path-relative URL
                    _ => root.join("").and_then(|r| r.join(&href)),
                };

                joined.map(|u| u.to_string()).ok()
            } else {
                Some(href)
            };

            match href {
                Some(h) => el.set_attribute("href", &h)?,
                None => el.remove_attribute("href"),
            }

            Ok(())
        }),

        // remove extraneous line breaks
        lol_html::element!("br", |el| {
            let prev = brs.fetch_add(1, Ordering::SeqCst);
            if prev > 0 || in_list.load(Ordering::SeqCst) {
                el.remove();
            }
            Ok(())
        }),

        // handle paragraphs
        lol_html::element!("*", |el| {
            if el.tag_name() == "br" {
                return Ok(());
            }

            let prev = brs.swap(0, Ordering::SeqCst);
            if prev >= 2 {
                println!("ending p and starting new p");
                el.before("</p><p>", ContentType::Html);
            }

            Ok(())
        }),
    ];

    let doc_handlers = DocumentContentHandlers::default()
        .text(|t| {
            let trimmed = t.as_str().trim().to_string();
            if !trimmed.is_empty() {
                let prev = brs.swap(0, Ordering::SeqCst);
                if prev >= 2 {
                    println!("ending p and starting new p");
                    t.before("</p><p>", ContentType::Html);
                }
            }

            println!("doc: {:?}", t.as_str());
            Ok(())
        });

    let rewritten = lol_html::rewrite_str(
        &info.inner_html(),
        RewriteStrSettings {
            element_content_handlers: rewriters,
            document_content_handlers: vec![doc_handlers],
            ..Default::default()
        },
    )
        .context("could not rewrite xma html")?;

    println!("rewritten");
    println!("{rewritten}");

    let parser = html5ever::parse_fragment(
        RcDom::default(),
        html5ever::ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("div")),
        Default::default(),
    );
    let dom = parser.one(&*rewritten);

    fn handle_handle(handle: &Handle, markdown: &mut String) {
        let mut push_after: Option<Cow<str>> = None;

        match &handle.data {
            NodeData::Element { name, attrs, .. } => {
                let attrs = attrs.borrow();
                let name = name.local.as_ref();
                match name {
                    "h2" => {
                        markdown.push_str("## ");
                        push_after = Some("\n".into());
                    }
                    "a" => {
                        if let Some(attr) = attrs.iter().find(|attr| attr.name.local.as_ref() == "href") {
                            markdown.push('[');
                            push_after = Some(format!("]({})", attr.value.as_ref()).into());
                        }
                    }
                    "p" => {
                        markdown.push('\n');
                    }
                    "strong" => {
                        markdown.push_str("**");
                        push_after = Some("**".into());
                    }
                    "em" => {
                        markdown.push('*');
                        push_after = Some("*".into());
                    }
                    "br" => {
                        markdown.push_str("  \n");
                    }
                    _ => {}
                }
            }
            NodeData::Text { contents: text } => {
                let text = text.borrow();
                let mut text = text.as_ref();

                let mut is_list_item = false;

                // convert jank lists to real lists
                if let Some(suffix) = text.trim_start().strip_prefix(|c| c == '◽' || c == '-') {
                    is_list_item = true;
                    text = suffix.trim_start();
                }

                if is_list_item {
                    markdown.push_str("- ");
                }

                markdown.push_str(text.as_ref());
            }
            _ => {}
        }

        for child in handle.children.borrow().iter() {
            handle_handle(child, markdown);
        }

        if let Some(after) = push_after {
            markdown.push_str(after.as_ref());
        }
    }

    let mut markdown = String::with_capacity(rewritten.len());
    handle_handle(&dom.document, &mut markdown);

    Ok(ConvertedInfo {
        title,
        nsfw,
        nsfl,
        description_md: markdown,
        tags,
    })
}

// #[derive(SimpleObject)]
    pub struct ConvertedInfo {
    pub title: String,
    pub nsfw: bool,
    pub nsfl: bool,
    pub description_md: String,
    pub tags: Vec<String>,
}
