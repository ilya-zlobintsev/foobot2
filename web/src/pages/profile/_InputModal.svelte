<script>
    import { closeModal } from "svelte-modals";
    // provided by Modals
    export let isOpen;

    export let title;
    export let input = "";
    export let onAccept;
</script>

{#if isOpen}
    <div role="dialog" class="modal">
        <div class="contents">
            <h2>{title}</h2>
            <input bind:value={input} />
            <div class="actions">
                <button on:click={closeModal}>Cancel</button>
                <button on:click={onAccept(input)}>Ok</button>
            </div>
        </div>
    </div>
{/if}

<style>
    .modal {
        position: fixed;
        top: 0;
        bottom: 0;
        right: 0;
        left: 0;
        display: flex;
        justify-content: center;
        align-items: center;

        /* allow click-through to backdrop */
        pointer-events: none;
    }

    .contents {
        min-width: 240px;
        border-radius: 6px;
        padding: 16px;
        background: rgb(61, 61, 61);
        display: flex;
        flex-direction: column;
        justify-content: space-between;
        pointer-events: auto;
    }

    h2 {
        text-align: center;
        font-size: 24px;
    }

    input {
        text-align: center;
        margin-top: 16px;
        background-color: black;
        border: 1px solid darkgray;
        border-radius: 3px;
    }

    .actions {
        margin-top: 32px;
        display: flex;
        justify-content: flex-end;
    }

    .actions button {
        background-color: black;
        margin-left: 5px;
        margin-right: 5px;
        padding: 3px;
        border: 1px solid darkgray;
        border-radius: 2px;
    }
</style>
