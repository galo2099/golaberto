document.observe("dom:loaded", function() {
  $$('.home_name', '.away_name', '.name').each(function(item) {
    item.observe('mouseenter', function(event) {
      var $this = $(this);
      if (this.offsetWidth < this.scrollWidth && !$this.readAttribute('title')){
        $this.writeAttribute('title', $this.innerText);
      }
    });
  });
});
