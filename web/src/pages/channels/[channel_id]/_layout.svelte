<script>
    import { isActive } from "@roxi/routify";
    import { getJson } from "../../../common";

    export let channel_id;
    export let channel_info = undefined;
    export let scoped;

    async function getChannelInfo() {
        channel_info = await getJson(`/api/channels/${channel_id}/info`);

        if (channel_info["permissions"]) {
            if (channel_info["permissions"]["value"] >= 5) {
                links.push(["./filters", "Filters"]);
            }
        }

        for (const section of channel_info["extra_sections"]) {
            links.push(section);
        }
    }

    $: {
        if (scoped["session_verified"]) {
            getChannelInfo();
        }
    }

    let links = [["./commands", "Commands"]];
</script>

{#if channel_info}
    <h1>
        Channel {channel_info.display_name || channel_info.id}
    </h1>
    <div class="channel-frame">
        <div class="sidebar-buttons">
            {#each links as [path, name]}
                <a class="link" class:active={$isActive(path)} href={path}>
                    {name}
                </a>
            {/each}
        </div>
        <div class="content">
            <slot
                scoped={{
                    channel_info,
                    session_verified: scoped["session_verified"],
                }}
            />
        </div>
    </div>
{/if}

<style>
    .channel-frame {
        margin: 0px 10%;
        display: flex;
    }

    .sidebar-buttons {
        display: flex;
        flex-direction: column;
        height: max-content;
        outline: 2px solid #1a1a1a;
        border-radius: 25pt;
    }

    .link {
        padding: 20px;
        text-decoration: none;
        border-radius: 25pt;
        color: #8e8e8e;
        font-size: 12pt;
    }

    .active {
        background-color: #88ffbc;
        color: #196239;
    }

    .content {
        margin: 15px;
        width: 100%;
    }

    :global(table) {
        width: 100%;
        border-collapse: collapse;
    }

    :global(table, td, th) {
        border: 1px solid #2f2f2f;
        background-color: #161616;
        font-size: 14pt;
    }

    :global(thead, tr) {
        height: 34pt;
    }
</style>
