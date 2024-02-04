$(document).ready(function() {
  if ($.inArray('editor', $.map(current_user.roles, function(role) { return role.name; })) === -1) {
    $('div[data-authorize]').hide();
  }
});
