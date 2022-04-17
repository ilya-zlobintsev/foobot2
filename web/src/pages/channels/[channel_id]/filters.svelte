<script>
    import { getJson } from "../../../common";

    export let scoped;
    export let channel_info;

    $: ({ channel_info } = scoped);
</script>

{#await getJson(`/api/channels/${channel_info.id}/filters`)}
    Loading...
{:then filters}
    {#if filters.length > 0}
        <table border="1">
            <col style="width:40%" />
            <col style="width:40%" />
            <col style="width:20%" />

            <thead>
                <th>Regex</th>
                <th>Replacement</th>
                <th>Block message</th>
            </thead>

            <tbody>
                {#each filters as filter}
                    <tr>
                        <td>{filter.regex}</td>
                        <td>{filter.replacement || ""}</td>
                        <td>{filter.block_message}</td>
                    </tr>
                {/each}
            </tbody>
        </table>
    {:else}
        No filters configured
    {/if}
{/await}

<style>
    table {
        width: 100%;
        border-collapse: collapse;
    }
</style>
