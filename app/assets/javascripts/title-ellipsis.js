jQuery(document).ready(function ($) {
  $('.home_name, .away_name, .name, table.dataTable tr td, .ellipsis, .championship_name').each(function () {
    $(this).on('mouseenter', function (event) {
      var $this = $(this);
      if (this.offsetWidth < this.scrollWidth && !$this.attr('title')) {
        $this.attr('title', $this.text());
      }
    });
  });
});
