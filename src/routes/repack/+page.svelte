<script lang='ts'>
    import { chooseTtmp } from '$lib/util';
    import { listen } from '@tauri-apps/api/event';
    import { invoke } from '@tauri-apps/api';

    interface DeduplicateProgressEmpty {
        kind: 'settingUp' | 'creatingArchive' | 'done';
    }

    interface DeduplicateProgressProcessingFiles {
        kind: 'processingFiles';
        current: number;
        total: number;
    }

    type DeduplicateProgress = DeduplicateProgressEmpty | DeduplicateProgressProcessingFiles;

    let processing = false;
    let progress: DeduplicateProgress | null = null;

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
        const path = await chooseTtmp();
        if (path === null) {
            return;
        }

        const unlisten = await listen('deduplicate-progress', p => {
            progress = p.payload as DeduplicateProgress;
        });

        try {
            await invoke('deduplicate', {
                path,
            });
        } finally {
            unlisten();
        }
    }

    function progressName(progress: DeduplicateProgress): string {
        switch (progress.kind) {
            case 'settingUp':
                return 'Setting things up';
            case 'processingFiles':
                return 'Processing files';
            case 'creatingArchive':
                return 'Creating new TTMP';
            case 'done':
                return 'Finished';
        }
    }
</script>

<article>
    <header>
        <h1>Deduplicate a TTMP</h1>
    </header>

    <p>
        By default, TexTools ends up duplicating files, i.e. including exactly the same file more than
        once, inside of TTMPs. This can lead to massive size bloat. This tool will remove the duplicate
        files for you and create a new TTMP in the same folder.
    </p>

    <p>
        <small>
            Note that Heliosphere does this for you automatically after upload, but if you want to
            reduce your TTMP's file size prior to uploading it, you can do so here.
        </small>
    </p>
</article>

{#if progress}
    {progressName(progress)}
    {#if progress.kind === 'processingFiles'}
        <progress value={progress.current} max={progress.total}></progress>
    {:else if progress.kind === 'done'}
        <progress value='1' max='1'></progress>
    {:else}
        <progress indeterminate></progress>
    {/if}
{/if}

<button on:click={choose} aria-busy={processing} disabled={processing}>Choose file</button>
