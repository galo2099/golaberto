function handleFileSelect(evt, preview_id) {
  if (!FileReader) return;
  var files = evt.target.files; // FileList object
  // Loop through the FileList and render image files as thumbnails.
  for (var i = 0, f; f = files[i]; i++) {
    // Only process image files.
    if (!f.type.match('image.*')) {
      continue;
    }
    var reader = new FileReader();
    // Closure to capture the file information.
    reader.onload = (function(theFile) {
      return function(e) {
        // Render thumbnail.
        var span = document.getElementById(preview_id);
        span.innerHTML = ['<img src="', e.target.result,
                          '" title="', escape(theFile.name), '"/>'].join('');
      };
    })(f);
    // Read in the image file as a data URL.
    reader.readAsDataURL(f);
  }
}

function addPreviewToFileInput(input_id, preview_id) {
  document.getElementById(input_id).addEventListener('change', function(evt) { handleFileSelect(evt, preview_id) }, false);
}
