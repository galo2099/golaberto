// This is a manifest file that'll be compiled into application.js, which will include all the files
// listed below.
//
// Any JavaScript/Coffee file within this directory, lib/assets/javascripts, vendor/assets/javascripts,
// or vendor/assets/javascripts of plugins, if any, can be referenced here using a relative path.
//
// It's not advisable to add code directly here, but if you do, it'll appear at the bottom of the
// the compiled file.
//
// WARNING: THE FIRST BLANK LINE MARKS THE END OF WHAT'S TO BE PROCESSED, ANY BLANK LINE SHOULD
// GO AFTER THE REQUIRES BELOW.
//
//= require boxover
//= require image_upload
//= require ability
//= require jstz
//= require title-ellipsis

// this is now my preferred way of dealing with confirmation dialog
// it is just much simpler than Turbo way
document.addEventListener("click", event => {
  const element = event.target.closest("[data-confirm]")

  if (element && !confirm(element.dataset.confirm)) {
    event.preventDefault()
  }
})
