module WillPaginate
  module ViewHelpers
    def will_paginate_with_localization(collection = nil, options = {})
      pagination_options[:previous_label] = _("&laquo; Previous")
      pagination_options[:next_label] = _("Next &raquo;")
      will_paginate_without_localization collection, options
    end
    alias_method_chain :will_paginate, :localization
  end
end

