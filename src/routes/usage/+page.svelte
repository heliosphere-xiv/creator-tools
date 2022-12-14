<script lang='ts'>
    import { GQL_GetMe } from '$houdini';
    import { chooseTtmp, formatBytes } from '$lib/util';
    import { invoke } from '@tauri-apps/api';
    import { listen } from '@tauri-apps/api/event';
    import { sep } from '@tauri-apps/api/path';
    import { sortBy, toPairs } from 'lodash-es';

    interface StorageProgressEmpty {
        kind: 'hashingTtmp' | 'calculatingSize' | 'done';
    }

    interface StorageProgressExtractingFiles {
        kind: 'extractingFiles';
        current: number;
        total: number;
    }

    type StorageProgress = StorageProgressEmpty | StorageProgressExtractingFiles;

    interface CalculatedUsage {
        total: number;
        hashes: Record<string, Calculated>;
        sorted: [string, Calculated][];
    }

    interface Calculated {
        size: number;
        files: string[];
        counts: boolean;
    }

    let processing = false;
    let file_name: string | null = null;
    let progress: StorageProgress | null = null;
    let size: CalculatedUsage | null = null;

    $: me = $GQL_GetMe.data?.me;
    $: totalStorage = me ? me.storageTier.totalSize + me.additionalStorage : 0;
    $: usedStorage = me ? me.usedStorage : 0;
    $: availableStorage = me ? totalStorage - usedStorage : 0;
    $: percentage = me && totalStorage > 0 ? usedStorage / totalStorage * 100 : 0;

    const formatter: Intl.NumberFormat = new Intl.NumberFormat([], {
        maximumFractionDigits: 2,
    });

    async function choose() {
        processing = true;
        file_name = null;
        progress = null;
        size = null;

        try {
            await inner();
        } finally {
            processing = false;
        }
    }

    async function inner() {
        const chosen = await chooseTtmp();
        if (chosen === null) {
            return;
        }

        const parts = (chosen as string).split(sep);
        file_name = parts.length > 0 ? parts[parts.length - 1] : null;

        const unlisten = await listen('storage-progress', p => {
            progress = p.payload as StorageProgress;
        });

        try {
            size = await invoke('calculate_usage', {
                path: chosen,
                hashes: me?.hashes || [],
            }) as CalculatedUsage;

            const pairs = toPairs(size.hashes);
            size.sorted = sortBy(pairs, x => -x[1].size);
        } finally {
            unlisten();
        }
    }

    function progressName(progress: StorageProgress) {
        switch (progress.kind) {
            case 'hashingTtmp':
                return 'Checking if this TTMP has been previously calculated';
            case 'extractingFiles':
                return 'Extracting and compressing files';
            case 'calculatingSize':
                return 'Calculating new usage size';
            case 'done':
                return 'Finished';
        }
    }

    function fileName(s: string): string {
        const parts = s.split('/');
        return parts[parts.length - 1];
    }
</script>

<article>
    <header>
        <h1>TTMP usage calcuation</h1>
    </header>

    <p>
        This tool will allow you to gauge how much of your account storage a TTMP will use after upload.
        {#if !me}
            To ensure accurate results, be sure to log in.
        {/if}
    </p>
</article>

{#if me}
    Current usage:<br />
    {formatBytes(usedStorage)} / {formatBytes(totalStorage)}
    ({formatter.format(percentage)}% used)
    <progress value={usedStorage} max={totalStorage}></progress>
{/if}

{#if size}
    <h1 class='size'>
        {formatBytes(size.total)}
    </h1>

    <p>
        {#if file_name}
            <code>{file_name}</code><br />
        {/if}

        That's {formatter.format(totalStorage === 0 ? 0 : size.total / totalStorage * 100)}% of your
        storage.

        {#if size.total > availableStorage}
            That is more space than you have available.
        {:else}
            You would have {formatBytes(totalStorage === 0 ? 0 : totalStorage - size.total)} left.
        {/if}
    </p>

    <details>
        <summary>Breakdown</summary>
        <table>
            <thead>
            <tr>
                <th>Hash</th>
                <th>Files</th>
                <th>Size</th>
            </tr>
            </thead>

            <tbody>
            {#each size.sorted as entry}
                <tr>
                    <td title={entry[0]}>{entry[0]}</td>
                    <td>
                        <div class='file-list'>
                            {#each entry[1].files as file}
                                <code title={file}>{fileName(file)}</code>
                            {/each}
                        </div>
                    </td>
                    <td title='{formatter.format(entry[1].size)} bytes'>
                        {formatBytes(entry[1].size)}
                    </td>
                </tr>
            {/each}
            </tbody>
        </table>
    </details>
{/if}

{#if progress}
    {progressName(progress)}
    {#if progress.kind === 'extractingFiles'}
        <progress value={progress.current} max={progress.total}></progress>
    {:else if progress.kind === 'done'}
        <progress value='1' max='1'></progress>
    {:else}
        <progress indeterminate></progress>
    {/if}
{/if}

<button on:click={choose} aria-busy={processing} disabled={processing}>Choose file</button>

<style lang='scss'>
  .size {
    margin-top: var(--spacing);
  }

  table {
    table-layout: fixed;
    width: 100%;

    td {
      width: calc(100% / 3);

      &, code {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      code {
        display: block;
        max-width: 100%;
      }

      .file-list {
        code + code {
          margin-top: var(--spacing);
        }
      }
    }
  }
</style>
