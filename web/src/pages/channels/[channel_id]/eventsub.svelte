<script>
    import { getJson } from "../../../common";

    export let scoped;

    export let mod;
    export let channel_info;

    $: {
        ({ channel_info } = scoped);

        const permissions = channel_info.permissions;
        mod = permissions ? permissions.value >= 5 : 0;
    }
</script>

{#await getJson(`/api/channels/${channel_info.id}/eventsub`)}
    Loading commands...
{:then triggers}
    {#if triggers.length > 0}
        <table border="1">
            <col style="width:30%" />
            <col style="width:30%" />
            <col style="width:30%" />
            {#if mod}
                <col style="width:10%" />
            {/if}

            <thead>
                <th>Event type</th>
                <th>Condition</th>
                <th>Action</th>
                {#if mod}
                    <th>Mod</th>
                {/if}
            </thead>

            <tbody>
                {#each triggers as trigger}
                    <tr>
                        <td>{trigger.event_type}</td>
                        <td>{trigger.condition}</td>
                        <td>{trigger.action || 0}s</td>
                        {#if mod}
                            <td> Mod actions </td>
                        {/if}
                    </tr>
                {/each}
            </tbody>
        </table>
    {:else}
        No EventSub triggers configured
    {/if}
{/await}

<style>
    table {
        border-collapse: collapse;
    }
</style>
