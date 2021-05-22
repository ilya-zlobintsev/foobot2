const base_url = window.location.origin;
const path_url = window.location.pathname;

const channel_id = path_url.split("/")[2]

const Http = new XMLHttpRequest();
const url = new URL(base_url + "/api/permissions");

url.searchParams.append("channel_id", channel_id);

Http.open("GET", url);
Http.send();

Http.onloadend = (e) => {
    if (Http.responseText == "channel_mod") {
        document.getElementById("add_command_button").disabled = false;
    }
}