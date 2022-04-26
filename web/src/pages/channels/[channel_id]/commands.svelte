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

<!-- TODO: maybe have long command actions scrollable or cut off? -->

{#await getJson(`/api/channels/${channel_info.id}/commands`)}
    Loading commands...
{:then commands}
    {#if commands.length > 0}
        <table id="commands_table" border="1">
            <col style="width:15%" />
            <col style="width:55%" />
            <col style="width:5%" />
            <col style="width:15%" />
            {#if mod}
                <col style="width:10%" />
            {/if}

            <thead>
                <th>Name</th>
                <th>Action</th>
                <th>Cooldown</th>
                <th>Permissions</th>
                {#if mod}
                    <th>Mod</th>
                {/if}
            </thead>

            <tbody>
                {#each commands as command}
                    <tr>
                        <td>{command.name}</td>
                        <td>{command.action}</td>
                        <td>{command.cooldown || 0}s</td>
                        <td>{command.permissions || "Everyone"}</td>
                        {#if mod}
                            <td> Mod actions </td>
                        {/if}
                    </tr>
                {/each}
            </tbody>
        </table>
    {:else}
        <h1>No commands!</h1>
    {/if}
{/await}
