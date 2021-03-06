#
# Distributed with Rails Date Kit
# http://www.methods.co.nz/rails_date_kit/rails_date_kit.html
#
# Author:  Stuart Rackham <srackham@methods.co.nz>
# License: This source code is released under the MIT license.
#
module DateHelper

  # Rails text_field helper plus drop-down calendar control for date input. Same
  # options as text_field plus optional :format option which accepts
  # same date display format specifiers as calendar_open() (%d, %e, %m, %b, %B, %y, %Y).
  # If the :format option is not set the the global Rails :default date format
  # is used or failing that  '%d %b %Y'.
  #
  # Explicitly pass it the date value to ensure it is formatted with desired format.
  # Example:
  #
  # <%= date_field('person', 'birthday', :value => @person.birthday) %>
  #
  def date_field(object_name, method, options={})
    options = set_date_options(options)
    text_field object_name, method, options
  end

  def date_field_tag(name, value = nil, options = {})
    options = set_date_options(options)
    text_field_tag name, value, options
  end

  private

  def set_date_options(options)
    format = options.delete(:format) ||
             ActiveSupport::CoreExtensions::Date::Conversions::DATE_FORMATS[:default] ||
             '%d %b %Y'
    if options[:value].is_a?(Date)
      options[:value] = options[:value].strftime(format)
    end
    months = (1..12).map{|i| GettextDate::Conversions.abbr_monthnames(i) }.collect { |m| "'#{m}'" }
    months = '[' + months.join(',') + ']'
    days = (0..6).map{|i| GettextDate::Conversions.abbr_daynames(i) }.collect { |d| "'#{d}'" }
    days = '[' + days.join(',') + ']'
    { :onfocus => "this.select();calendar_open(this,{format:'#{format}',images_dir:'/images',month_names:#{months},day_names:#{days}})",
      :onclick => "event.cancelBubble=true;this.select();calendar_open(this,{format:'#{format}',images_dir:'/images',month_names:#{months},day_names:#{days}})",
    }.merge(options);
  end
end
