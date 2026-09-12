// The theme picker.
//
// The choice itself is applied by one attribute on <html>, which
// `assets/css/nikaia.css` reads; the layout already restored a stored choice
// before the first paint. What is left for this file is the control: reveal
// it, show which theme is in force, write a new one down, and keep two tabs
// of the same site from disagreeing.
//
// "system" is the absence of the attribute rather than a value of it, so that
// a reader who never touches the control, one who has no script, and one who
// chooses System all end up in the same state.

(function () {
  "use strict";

  var KEY = "nikaia-theme";
  var CHOICES = ["system", "light", "dark", "sepia"];

  var picker = document.querySelector(".theme-picker");
  if (!picker) return;

  function stored() {
    try {
      var value = window.localStorage.getItem(KEY);
      return CHOICES.indexOf(value) > 0 ? value : "system";
    } catch (e) {
      // Storage denied. The control still works for this page view.
      return document.documentElement.getAttribute("data-theme") || "system";
    }
  }

  function show(choice) {
    if (choice === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", choice);
    }

    var buttons = picker.querySelectorAll("[data-theme-choice]");
    for (var i = 0; i < buttons.length; i++) {
      var pressed = buttons[i].getAttribute("data-theme-choice") === choice;
      buttons[i].setAttribute("aria-pressed", pressed ? "true" : "false");
    }
  }

  picker.addEventListener("click", function (event) {
    var button = event.target.closest("[data-theme-choice]");
    if (!button) return;

    var choice = button.getAttribute("data-theme-choice");
    show(choice);

    try {
      if (choice === "system") {
        window.localStorage.removeItem(KEY);
      } else {
        window.localStorage.setItem(KEY, choice);
      }
    } catch (e) {
      // Nothing to do: the choice holds for this page and is not remembered.
    }
  });

  // Another tab of this site changed the setting. `storage` fires only in the
  // tabs that did not make the change, which is exactly the set that needs it.
  window.addEventListener("storage", function (event) {
    if (event.key === KEY || event.key === null) show(stored());
  });

  show(stored());
  picker.hidden = false;
})();
