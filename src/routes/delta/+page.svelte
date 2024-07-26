<script lang='ts'>
    import { type GetMod$result, GQL_CheckVanityUrl, GQL_GetFiles, GQL_GetMod } from '$houdini';
    import { base32ToUuid, chooseMod, formatBytes } from '$lib/util';
    import CoverImage from '$lib/coverImage.svelte';
    import { listen } from '@tauri-apps/api/event';
    import { invoke } from '@tauri-apps/api';

    type Package = GetMod$result['package'];
    type Variant = NonNullable<Package>['variants'][number];
    type Version = Variant['versions'][number];

    interface DeltaProgressEmpty {
        kind: 'settingUp' | 'calculatingDifference' | 'done';
    }

    interface DeltaProgressCount {
        kind: 'hashingFiles' | 'creatingUpdateFile';
        current: number;
        total: number;
    }

    type DeltaProgress = DeltaProgressEmpty | DeltaProgressCount;

    let rawUrl = '';
    let variant: Variant | null;
    let variants: Variant[] = [];
    let variantIdx = -1;
    let version: Version | null;
    let versionIdx = -1;
    let mod: Promise<Package | null>;
    let processing = false;
    let progress: DeltaProgress | null = null;

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

        let id;
        if (parts[2].length !== 26) {
            const resp = await GQL_CheckVanityUrl.fetch({
                variables: {
                    slug: parts[2],
                },
            });

            id = resp.data?.checkVanityUrl;
        } else {
            id = base32ToUuid(parts[2]);
        }

        if (id === null || id === undefined) {
            return null;
        }

        const resp = await GQL_GetMod.fetch({
            variables: {
                id,
            },
        });

        console.log(resp);

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

            versionIdx = info === null ? -1 : 0;

            progress = null;

            return Promise.resolve(info);
        });
    }

    function progressName(progress: DeltaProgress): string {
        switch (progress.kind) {
            case 'settingUp':
                return 'Setting things up';
            case 'hashingFiles':
                return 'Processing files';
            case 'calculatingDifference':
                return 'Calculating differences';
            case 'creatingUpdateFile':
                return 'Creating delta update';
            case 'done':
                return 'Finished';
        }
    }

    async function choose() {
        processing = true;
        progress = null;

        try {
            await inner();
        } finally {
            processing = false;
        }
    }

    async function inner() {
        if (version === null) {
            return;
        }

        const resp = await GQL_GetFiles.fetch({
            variables: {
                id: version.id,
            },
        });
        const neededFiles = resp.data?.getVersion?.neededFiles;
        if (neededFiles === undefined) {
            // FIXME: show error
            return;
        }

        const path = await chooseMod();
        if (path === null) {
            return;
        }

        const unlisten = await listen('delta-progress', p => {
            progress = p.payload as DeltaProgress;
        });

        try {
            await invoke('delta', {
                path,
                info: {
                    version_id: version.id,
                    needed_files: neededFiles,
                },
            });
        } catch (e) {
            console.log(e);
            throw e;
        } finally {
            unlisten();
        }
    }
</script>

<article>
    <header>
        <h1>Delta update creator</h1>
    </header>

    <p>
        If you'd prefer to only upload the files that actually changed between a previous version
        and the next version, you can use this tool to generate a <em>delta update file</em>.
    </p>

    <p>
        Paste in a mod URL from Heliosphere, choose the version that your update comes after, then
        provide your mod. This tool will figure out the differences and provide you with a delta
        update file that you can upload on Heliosphere.
    </p>

    <strong>
        This tool currently only works with Penumbra modpacks (<code>.pmp</code>).
    </strong>
</article>

<label>
    Mod URL
    <input
        type='url'
        placeholder='https://heliosphere.app/mod/xxxxxxxxxxxxxxxxxxxxxxxxxx'
        bind:value={rawUrl}
        on:input={updateInfo}
        required
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
                        {#each variant.versions as version, idx (version.id)}
                            <option value={idx}>{version.version}</option>
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
    {progressName(progress)}
    {#if progress.kind === 'hashingFiles' || progress.kind === 'creatingUpdateFile'}
        <progress value={progress.current} max={progress.total}></progress>
    {:else if progress.kind === 'done'}
        <progress value='1' max='1'></progress>
    {:else}
        <progress></progress>
    {/if}
{/if}

{#if version}
    {#if !progress}
        <p>
            Choose the mod archive that updates version {version.version}.
        </p>
    {/if}

    <button
        on:click={choose}
        aria-busy={processing}
        disabled={processing}
    >
        Choose file
    </button>
{/if}

<style lang='scss'>
  .mod-card {
    & > header {
      aspect-ratio: 16 / 9;
      position: relative;
      background-color: black;
      padding: 0;
    }
  }

  button {
    width: 100%;
    margin-bottom: var(--pico-spacing);
  }
</style>
