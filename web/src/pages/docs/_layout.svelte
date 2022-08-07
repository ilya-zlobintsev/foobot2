<div id="md-content">
    <slot />
</div>

<style>
    #md-content {
        margin-left: 10%;
        margin-right: 10%;
        text-align: start;
    }

    :global(p),
    :global(h1),
    :global(h2),
    :global(h3),
    :global(ul),
    :global(pre) {
        padding: 5px;
    }

    :global(ul) {
        padding-left: 40px;
    }
</style>
