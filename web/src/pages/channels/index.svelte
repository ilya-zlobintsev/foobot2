<script>
    import { getJson } from "../../common";
</script>

{#await getJson("/api/channels")}
    <h1>Loading channels...</h1>
{:then channels}
    <div class="channels">
        {#each channels as channel}
            <a class="channel" href="./channels/{channel.id}/commands">
                <h1 class="platform_text">
                    Platform:
                    <span class="platform_subtext">{channel.platform}</span>
                </h1>
                <p class="channel_id">Channel: {channel.channel}</p>
                <p class="channel_name">
                    {#if channel.display_name} ({channel.display_name}){/if}
                </p>
                <p class="channel_local_id">ID: {channel.id}</p>
            </a>
        {/each}
    </div>
{/await}

<style>
    .channels {
        display: flex;
        flex-wrap: wrap;
        justify-content: center;
    }

    .channel {
        border: 2px solid #1e1e1e;
        border-radius: 14pt;
        padding: 20pt;
        margin: 0.7em;
        width: 250px;
        display: flex;
        flex-direction: column;
        align-content: space-around;
        justify-content: space-between;
        text-decoration: none;
        box-shadow: 0px 10px 12px #0d0d0d;
    }

    .channel:hover {
        box-shadow: 0px 2px 6px 6px #535353;
        outline: 1px solid white;
    }

    .channel_name {
        font-size: 14pt;
    }

    .platform_text {
        color: #cacaca;
        font-size: 22pt;
        font-weight: 600;
    }
    .platform_subtext {
        color: #a8a8a8;
        font-size: 22pt;
        font-weight: 400;
    }

    .channel_id {
        color: #8a8a8a;
        font-weight: 400;
        font-size: 14pt;
    }

    .channel_local_id {
        color: #d5d5d5;
        font-size: 16pt;
        font-weight: 600;
        margin-top: 6pt;
    }
</style>
