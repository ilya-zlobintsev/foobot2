<script>
    import { getJson } from "../common";
</script>

<h1 class="main-big">Hello!</h1>
<h2 class="main-medium">
    The bot is currently registered in
    <h2 class="main-highlighted">
        {#await getJson("/api/channels/count")}
            loading...
        {:then count}
            {count}
        {/await}
    </h2>
    channels.
</h2>
<h2 class="main-small">
    You can take a look at the list
    <a class="main-highlighted" href="/channels">here</a>
    .
</h2>

<style>
    .main-big {
        font-size: 5.5em;
    }

    .main-medium {
        color: #bbb;
        font-weight: 300;
        line-height: 1.5em;
    }

    .main-small {
        color: #848484;
        font-weight: 300;
        font-size: 1.2em;
    }
    .main-highlighted {
        color: #cecece;
        font-weight: 600;
        font-size: 1.1em;
        display: inline-block;
    }
</style>
