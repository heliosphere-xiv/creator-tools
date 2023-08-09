<script lang='ts'>
    export let imageUrl: string | undefined | null;

    let bgImage = imageUrl ? `url(${imageUrl})` : null;
</script>

<div class='outer'>
    <div class='inner' style:background-image={bgImage}>
    </div>
    <div class='image'>
        {#if imageUrl}
            <img src={imageUrl} loading='lazy' alt='cover'/>
        {:else}
            <em>No cover image uploaded</em>
        {/if}
    </div>
</div>

<style lang='scss'>
  $transition-time-type: 100ms linear;
  $nsfw-transition: filter $transition-time-type;

  div.outer {
    position: relative;
    overflow: hidden;
    height: 100%;
    width: 100%;
    border-top-left-radius: var(--pico-border-radius);
    border-top-right-radius: var(--pico-border-radius);
  }

  div.inner {
    position: relative;
    height: 100%;
    width: 100%;
    filter: blur(10px);
    pointer-events: none;
    background-size: cover;
    background-position: center;
  }

  img {
    max-height: 100%;
    object-fit: contain;
    flex-grow: 1;
    filter: drop-shadow(0 0 0.5rem black);
    transition: filter 100ms linear;
  }

  .image {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    color: currentColor;
    width: 100%;
    height: 100%;

    position: absolute;
    top: 0;
    left: 0;

    &:hover, &:focus, &:active {
      background-color: inherit;
      text-decoration: none;
    }
  }

  em {
    margin: 0;
    padding: 0;
    align-self: center;
    user-select: none;
  }
</style>
