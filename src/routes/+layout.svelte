<script lang='ts'>
    import { beforeNavigate } from '$app/navigation';

    let details: HTMLDetailsElement;

    beforeNavigate(() => {
        details?.removeAttribute('open');
        (document.activeElement as HTMLElement).blur();
    });
</script>

<nav>
    <ul>
        <li><strong><a class='index' href='/'>Heliosphere</a></strong></li>
    </ul>
    <ul>
        <li>
            <details bind:this={details} role='list' dir='rtl'>
                <summary aria-haspopup='listbox' role='link'>
                    Tools
                </summary>
                <ul role='listbox'>
                    <li><a href='/usage'>Storage usage calculation</a></li>
                    <li><a href='/repack'>Local TTMP deduplication</a></li>
                    <li><a href='/download'>Remote TTMP download</a></li>
                </ul>
            </details>
        </li>
    </ul>
</nav>

<slot />

<style lang='scss'>
  .top-banner {
    padding: 0.5rem 1rem;
    color: var(--primary-inverse);
    background-color: var(--primary);
    border-bottom-left-radius: var(--border-radius);
    border-bottom-right-radius: var(--border-radius);
    margin-bottom: 0.5rem;

    a {
      --primary: var(--primary-inverse);
      text-decoration: underline;
    }
  }

  nav a.index {
    display: inline-flex;
    justify-content: center;
    align-items: center;

    &::before {
      --size: 1.5rem;

      display: inline-block;
      content: '';
      width: var(--size);
      height: var(--size);
      background: url('/favicon.png') no-repeat 0 0;
      background-size: var(--size);
      margin-right: 1ch;
    }
  }
</style>
