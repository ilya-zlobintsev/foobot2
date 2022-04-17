<script>
    import Cookies from "js-cookie";
    import { getJson } from "../common";

    async function getSession() {
        if (Cookies.get("session_id")) {
            session = await getJson("/api/session");
        }
        session_verified = true;
    }

    async function logout() {
        const response = await fetch("/api/session/logout", {
            method: "POST",
        });
        if (response.ok) {
            Cookies.remove("session_id");
            session = {};
        }
    }

    export let session = {};
    export let session_verified = false;

    let open = false;

    const login_platforms = Object.entries({
        Twitch: "/authenticate/twitch",
        Discord: "/authenticate/discord",
    });
</script>

<header>
    <div class="header-buttons">
        <h3 class="logo_text">FOOBOT2</h3>
        <a href="/">Home</a>
        <a href="/channels">Channels</a>
        <a href="/api/doc" target="_self">API</a>
    </div>
    <div class="user-section">
        {#await getSession()}
            Loading...
        {:then}
            {#if session.username}
                <a class="profile-href" href="/profile">
                    <h1 class="profile-text">{session.username}</h1>
                </a>
                <button class="logout" on:click={logout}>Log out</button>
            {:else}
                <div class="dropdown">
                    <button
                        class="dropbtn"
                        on:click={() => {
                            open = !open;
                        }}>Log in</button
                    >
                    {#if open}
                        <div class="dropdown-content">
                            {#each login_platforms as [platform, link]}
                                <a
                                    href="{link}?redirect_to={window.location
                                        .pathname}"
                                    target="_self">{platform}</a
                                >
                            {/each}
                        </div>
                    {/if}
                </div>
            {/if}
        {/await}
    </div>
</header>

<div class="content">
    <slot scoped={{ session, session_verified }} />
</div>

<style>
    :global(*) {
        font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue",
            "Segoe UI", "Selawik", "Open Sans", sans-serif;
        color: #f0f0f0;
        margin: 0px;
        padding: 0px;
    }

    .content {
        width: 100%;
        height: 100%;
        display: flex;
        margin-top: 3em;
        flex-direction: column;
        text-align: center;
    }

    header {
        display: flex;
        background-color: #161616;
        border-bottom: 1px solid #343434;
        height: 4em;
    }

    .header-buttons {
        display: flex;
        justify-content: left;
        flex-direction: row;
        width: 100%;
    }

    .header-buttons a {
        display: block;
        text-decoration: none;
        display: flex;
        align-items: center;
        padding: 10px;
    }

    header a:hover {
        background-color: #343434;
    }

    .user-section {
        display: flex;
        margin: 10px;
    }

    .profile-href {
        margin-right: 16px;
        display: flex;
        align-items: center;
        text-decoration: none;
        width: calc(
            100% - 2em
        ); /* TODO: replace this lidl button width fix with something thats better */
        justify-content: center;
        border-radius: 6px;
        outline: 1px solid #2f2f2f;
    }
    .profile-text {
        color: #b0b0b0;
        font-size: 14pt;
    }

    .logout {
        width: 7.5em;
        border: none;
        outline: 1px solid #2f2f2f;
        background-color: #161616;
        border-radius: 6px;
        color: #ff4343;
        transition: 0.25s;
        cursor: pointer;
    }

    .logout:hover {
        background-color: #351818;
    }

    .logo_text {
        width: 6.2em;
        height: 100%;
        color: #f0f0f0;
        font-size: 1.3em;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    /* Dropdown Button */
    .dropbtn {
        background-color: rgb(73, 73, 230);
        font-size: 16px;
        border: none;
        border-radius: 5px;
        height: 100%;
        white-space: nowrap;
        min-width: 100px;
        cursor: pointer;
    }

    /* The container <div> - needed to position the dropdown content */
    .dropdown {
        position: relative;
        display: inline-block;
    }

    /* Dropdown Content (Hidden by Default) */
    .dropdown-content {
        display: block;
        position: absolute;
        background-color: #161616;
        min-width: 100px;
        box-shadow: 0px 8px 16px 0px rgba(6, 6, 6, 0.6);
        z-index: 1;
        border-radius: 5px;
    }

    /* Links inside the dropdown */
    .dropdown-content a {
        padding: 12px 16px;
        text-decoration: none;
        display: block;
    }

    /* Change color of dropdown links on hover */
    .dropdown-content a:hover {
        background-color: rgb(59, 59, 59);
    }

    /* Show the dropdown menu on hover */
    .dropdown:hover .dropdown-content {
        display: block;
    }
</style>
