# Methods added to this helper will be available to all templates in the application.
module ApplicationHelper
  def pagination_links_remote(paginator, page_options={}, ajax_options={}, html_options={})
    pagination_links_each(paginator, page_options) {|page|
      ajax_options[:url].merge!({:page => page})
      link_to_remote(page, ajax_options, html_options)}
  end

  def unless_nil(value)
    yield value unless value.nil?
  end
end
