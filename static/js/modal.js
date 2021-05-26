const modal = document.getElementById("modal");
const show_modal_button = document.getElementById("show-modal-button");
const close_modal_button = document.getElementById("close-modal-button");

show_modal_button.onclick = function() {
    modal.style.display = "block";
}

close_modal_button.onclick = closeModal;

window.onclick = function(event) {
    if (event.target == modal) {
        closeModal();
    }
}

function closeModal() {
    modal.style.display = "none";
}