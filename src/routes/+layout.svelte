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
            <details bind:this={details} class='dropdown'>
                <summary aria-haspopup='listbox' role='link'>
                    Tools
                </summary>
                <ul role='listbox' dir='rtl'>
                    <li><a href='/delta'>Delta update creator</a></li>
                </ul>
            </details>
        </li>
    </ul>
</nav>

<slot />

<style lang='scss'>
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

  details > summary::after {
    height: 1rem;
  }
</style>
