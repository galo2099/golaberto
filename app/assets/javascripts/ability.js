document.observe("dom:loaded", function() {
  if (!$A(current_user.roles).pluck('name').include('editor')) {
    $$('div[data-authorize]').invoke('hide');
  }
});
