<script lang='ts'>
    import { type GetMod$result, GQL_GetFiles, GQL_GetMod } from '$houdini';
    import { base32ToUuid, formatBytes, uuidToBase32 } from '$lib/util';
    import { save } from '@tauri-apps/api/dialog';
    import { listen } from '@tauri-apps/api/event';
    import { invoke } from '@tauri-apps/api';
    import CoverImage from '$lib/coverImage.svelte';

    interface TtmpProgressCreatingManifest {
        kind: 'creatingManifest';
    }

    interface TtmpProgressDownloadingFiles {
        kind: 'downloadingFiles';
        current: number;
        total: number;
    }

    interface TtmpProgressProcessingArchive {
        kind: 'processingArchive';
    }

    interface TtmpProgressDone {
        kind: 'done';
    }

    type TtmpProgress = TtmpProgressCreatingManifest
        | TtmpProgressDownloadingFiles
        | TtmpProgressProcessingArchive
        | TtmpProgressDone;

    function progressName(progress: TtmpProgress): string {
        switch (progress.kind) {
            case 'creatingManifest':
                return 'Creating TTMP manifest';
            case 'downloadingFiles':
                return 'Downloading and processing files';
            case 'processingArchive':
                return 'Creating TTMP';
            case 'done':
                return 'Finished';
        }
    }

    type Package = GetMod$result['package'];
    type Variant = Package['variants'][number];
    type Version = Variant['versions'][number];

    let rawUrl = '';
    let variant: Variant | null;
    let variants: Variant[] = [];
    let variantIdx = -1;
    let version: Version | null;
    let versionIdx = -1;
    let mod: Promise<Package | null>;
    let processing = false;
    let progress: TtmpProgress | null = null;

    $: variant = variantIdx < 0 || variantIdx > variants.length ? null : variants[variantIdx];
    $: version = variant === null || versionIdx < 0 || versionIdx >= variant.versions.length
        ? null
        : variant.versions[versionIdx];

    async function getInfo(u: string): Promise<Package | null> {
        if (u === '') {
            return null;
        }

        const url = new URL(u);
        const parts = url.pathname.split('/');
        if (parts[1] !== 'mod') {
            // TODO: errors
            return null;
        }

        const id = base32ToUuid(parts[2]);

        const resp = await GQL_GetMod.fetch({
            variables: {
                id,
            },
        });

        const pkg = resp.data?.package;
        if (pkg === null || pkg === undefined) {
            // TODO: errors
            return null;
        }

        return pkg;
    }

    async function updateInfo() {
        mod = getInfo(rawUrl?.trim() || '').then(info => {
            variantIdx = info === null ? -1 : 0;
            variants = info?.variants || [];

            versionIdx = info === null ? -1 : info.variants[variantIdx].versions.length - 1;

            progress = null;

            return Promise.resolve(info);
        });
    }

    async function submit() {
        processing = true;

        try {
            await inner();
        } finally {
            processing = false;
        }
    }

    async function inner() {
        const pkg: Package = await mod;
        if (pkg === null || version === null) {
            return;
        }

        const resp = await GQL_GetFiles.fetch({
            variables: {
                id: version.id,
            },
        });
        if (resp.data === null) {
            return;
        }
        const ver = resp.data.getVersion;
        if (ver === null) {
            return;
        }

        const path = await save({
            title: 'Save TTMP',
            defaultPath: `${pkg.name} (${variant.name}).ttmp2`,
            filters: [{ name: 'TexTools Mod Packs', extensions: ['ttmp2', 'ttmp'] }],
        });

        const unlisten = await listen('ttmp-progress', p => {
            progress = p.payload as TtmpProgress;
        });

        try {
            await invoke('create_ttmp', {
                path,
                info: {
                    name: pkg.name,
                    author: pkg.user.username,
                    version: version.version,
                    description: pkg.description,
                    url: `https://heliosphere.app/mod/${uuidToBase32(pkg.id)}`,
                },
                groups: ver.groups,
                neededFiles: ver.neededFiles,
            });
        } catch (e) {
            console.log(e);
        } finally {
            unlisten();
        }
    }
</script>

<h1>Download as TTMP</h1>

<article>
    <p>
        Heliosphere mods do not use TTMPs internally, so this tool will
        construct a TTMP from a Heliosphere mod if you need one.
    </p>
</article>

<form on:submit|preventDefault={submit}>
    <label>
        Mod URL
        <input
            type='text'
            bind:value={rawUrl}
            on:input={updateInfo}
            required
            placeholder='https://heliosphere.app/mod/xxxxxxxxxxxxxxxxxxxxxxxxxx'
        />
    </label>

    {#await mod}
        <p>
            <em aria-busy='true'>Fetching mod info...</em>
        </p>
    {:then mod}
        {#if mod}
            <article class='mod-card'>
                {#if mod.images.length > 0}
                    <header>
                        <CoverImage
                            imageUrl='https://heliosphere.app/api/web/package/{mod.id}/image/{mod.images[0].id}' />
                    </header>
                {/if}
                <hgroup>
                    <h2>{mod.name}</h2>
                    <h3>{mod.tagline}</h3>
                </hgroup>

                <details>
                    <summary>Description</summary>
                    <div class='description'>
                        {mod.description}
                    </div>
                </details>

                {#if variants.length > 1}
                    <label>
                        Variant
                        <select bind:value={variantIdx}>
                            {#each mod.variants as variant, idx (variant.id)}
                                <option value={idx}>{variant.name}</option>
                            {/each}
                        </select>
                    </label>
                {/if}

                {#if variant}
                    <label>
                        Version
                        <select bind:value={versionIdx}>
                            {#each [...variant.versions].reverse() as version, idx (version.id)}
                                <option value={variant.versions.length - idx - 1}>{version.version}</option>
                            {/each}
                        </select>
                    </label>
                {/if}

                {#if version}
                    <table>
                        <tbody>
                        <tr>
                            <td>Download size</td>
                            <td>{formatBytes(version.downloadSize)}</td>
                        </tr>
                        <tr>
                            <td>Space required</td>
                            <td>{formatBytes(version.installSize)}</td>
                        </tr>
                        <tr>
                            <td>Total downloads</td>
                            <td>{new Intl.NumberFormat().format(mod.downloads)}</td>
                        </tr>
                        <tr>
                            <td>{version.version} downloads</td>
                            <td>{new Intl.NumberFormat().format(version.downloads)}</td>
                        </tr>
                        </tbody>
                    </table>

                    {#if version.changelog}
                        <details open class='changelog'>
                            <summary>{version.version} changelog</summary>
                            <pre>{version.changelog}</pre>
                        </details>
                    {/if}
                {/if}
            </article>
        {/if}
    {:catch e}
        <em>Error: {e}</em>
    {/await}

    {#if progress}
        <span>{progressName(progress)}</span>
        {#if progress.kind === 'downloadingFiles'}
            <progress value={progress.current} max={progress.total}></progress>
        {:else if progress.kind === 'done'}
            <progress value='1' max='1'></progress>
        {:else}
            <progress indeterminate></progress>
        {/if}
    {/if}

    <button type='submit' aria-busy={processing} disabled={version === null || processing}>Create TTMP</button>
</form>

<style lang='scss'>
  .mod-card {
    & > header {
      height: 18rem;
      padding: 0;

      & > img {
        border-top-left-radius: var(--border-radius);
        border-top-right-radius: var(--border-radius);

        object-fit: cover;
      }
    }

    .changelog {
      pre {
        padding: var(--spacing);
        white-space: pre-wrap;
        margin: 0;
      }
    }
  }

</style>
