<script>
    import { redirect } from "@roxi/routify";
    import { getJson } from "../../common";
    import { Modals, openModal, closeModal } from "svelte-modals";
    import InputModal from "./_InputModal.svelte";

    export let scoped;
    $: {
        let session_verified, session;
        ({ session, session_verified } = scoped);
        if (session_verified && !session.user_id) {
            $redirect("/");
        }
    }

    export let user = undefined;

    async function getUser() {
        user = await getJson("/api/session/user");
    }

    async function setLastfmName() {
        openModal(InputModal, {
            title: "Enter Last.FM username",
            input: user.lastfm_name,
            onAccept: async (name) => {
                if (name) {
                    await fetch("/api/session/lastfm", {
                        method: "POST",
                        body: name,
                    });

                    user.lastfm_name = name;
                    console.log(user);

                    closeModal();
                }
            },
        });
    }

    async function disconectSpotify() {
        await fetch("/api/session/spotify", {
            method: "DELETE",
        });

        user.spotify_connected = false;
    }
</script>

<Modals>
    <div slot="backdrop" class="backdrop" on:click={closeModal} />
</Modals>

{#await getUser()}
    Loading...
{:then}
    <h2>User info</h2>
    <div class="user">
        <div>
            <b>ID: {user.id}</b>
        </div>
        <div>
            <b>Twitch:</b>
            {#if user.twitch_user}
                {user.twitch_user.display_name} [{user.twitch_user.id}]
                <a href="/authenticate/twitch/manage" target="_self"
                    >Authorize channel manage</a
                >
            {:else}
                Not connected (<a href="/authenticate/twitch" target="_self"
                    >Connect</a
                >)
            {/if}
        </div>
        <div>
            <b>Discord:</b>
            {#if user.discord_user}
                {user.discord_user.username} [{user.discord_user.id}]
            {:else}
                Not connected (<a href="/authenticate/discord" target="_self"
                    >Connect</a
                >)
            {/if}
        </div>
        <div>
            <b>Last.FM:</b>
            {#if user.lastfm_name}
                {user.lastfm_name}
                <button on:click={setLastfmName}>Change</button>
            {:else}
                Not set <button on:click={setLastfmName}>Set</button>
            {/if}
        </div>
        <div>
            <b>Spotify:</b>
            {#if user.spotify_connected}
                Connected <button on:click={disconectSpotify}>Disconnect</button
                >
            {:else}
                <a href="/authenticate/spotify" target="_self">Connect</a>
            {/if}
        </div>
        {#if user.admin}
            <h2>Admin:</h2>
            <div>
                <a href="/authenticate/twitch/bot" target="_self"
                    >Authenticate Twitch bot</a
                >
            </div>
        {/if}
    </div>
{/await}

<style>
    a {
        color: lightblue;
    }

    button {
        width: 5em;
        border: none;
        outline: 1px solid #6e6e6e;
        background-color: #161616;
        border-radius: 6px;
        margin: 5px;
        cursor: pointer;
    }

    button:hover {
        background-color: #6e6e6e;
    }

    .backdrop {
        position: fixed;
        top: 0;
        bottom: 0;
        right: 0;
        left: 0;
        background: rgba(0, 0, 0, 0.5);
    }
</style>
