const base_url = window.location.origin;
const path_url = window.location.pathname;

const add_command_button = document.getElementById("show-modal-button");

const channel_id = path_url.split("/")[2]

const Http = new XMLHttpRequest();
const url = new URL(base_url + "/api/permissions");

url.searchParams.append("channel_id", channel_id);

Http.open("GET", url);
Http.send();

Http.onloadend = (e) => {
    if (Http.responseText == "channel_mod") {
        add_command_button.disabled = false;

        const commands_table = document.getElementById("commands_table");

        commands_table.innerHTML += "<col style=\"width:5%\">"

    }
}