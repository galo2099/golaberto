# Methods added to this helper will be available to all templates in the application.
module ApplicationHelper
  def pagination_links_remote(paginator, ajax_options={}, page_options={}, html_options={})
    page_options = { :window_size => 8 }.merge(page_options)
    html = ""
    unless paginator.current.first?
      ajax_options[:url].merge!({:page => paginator.current.previous.number})
      html << link_to_remote("Prev", ajax_options, html_options)
      html << " "
    else
      html << "Prev "
    end
    html << pagination_links_each(paginator, page_options) do |page|
      ajax_options[:url].merge!({:page => page})
      link_to_remote(page, ajax_options, html_options)
    end
    unless paginator.current.last?
      ajax_options[:url].merge!({:page => paginator.current.next.number})
      html << " "
      html << link_to_remote("Next", ajax_options, html_options)
    else
      html << " Next"
    end
    html
  end

  def unless_nil(value)
    yield value unless value.nil?
  end
 
  # Makes a vertical bar graph with several sets of bars.
  #
  # NOTE: Normalizes data to fit in the viewable area instead of being fixed to 100.
  #
  # Example:
  ## <% @data_for_graph = [[['January',10],['February',25],['March',45]],[['January',34],['February',29],['March',80]]] %>
  ## <%= bar_graph (@data_for_graph,{:width => 640,:height => 480}) do |index,variable|
  ##                 url_for( :action => 'report', :month => index) 
  ##               end
  ## %>
  #
  # alldata should be an array of variables, each one an array itself, of the form:
  ##    [['label1',value1],['label2',value2]]
  #
  # options hash:
  #*   :display_value_on_bar   if set to true, will display the value on top each bar, default behavior is not to show the value
  #*   :colors  is an array of colors in hex format: '#EEC2D2' if you don't set them, default colors will be used
  #*   :width and :height set the dimensions, wich default to 378x150
  #
  # url_creator_block:
  # 
  ##  the url_creator_block receives two parameters, index and variable, that are used to build the bars links. 
  ##  index is the position for this bar's that in its variable array, while variable is the variable this bar represents

  def multi_bar_graph(data=[], options={}, &url_creator)
    graph_id = (rand*10000000000000000000).to_i.to_s #we need a unique id for each graph on the page so we can have distinct styles for each of them
    if !options.nil? && options[:width] && options[:height]
      width,height=options[:width].to_i,options[:height].to_i
    else
      width,height = 378,150
    end
    colors = (%w(#ce494a #efba29 #efe708 #5a7dd6 #73a25a))*10 unless colors=options[:colors]
    floor_cutoff = 24 # Pixels beneath which values will not be drawn in graph
    data_max = options[:max] || data.max{ |a,b| a[:value] <=> b[:value] }[:value]
    bar_offset = 1 #originally set to 24
    bar_group_width = (width - bar_offset).to_f / data.size #originally set to 48px
    bar_increment = bar_group_width #originally set to 75
    bar_width = (bar_group_width - bar_offset) #originally set to 28px
    bar_image_offset = bar_offset + 1 #originally set to 28
    bar_padding = 2

    #p "bar_group_width =#{bar_group_width}"
    #p "bar_width = #{bar_width}"
    #p "bar_increment = #{bar_increment}"

    html = <<-"HTML"
    <style>
      #vertgraph-#{graph_id} {    				
          width: #{width}px; 
          height: #{height}px; 
          position: relative; 
          background-color: #ffffff;
          border: 1px solid #999999;
          font-family: "Lucida Grande", Verdana, Arial;
      }

      #vertgraph-#{graph_id} dl dd {
        position: absolute;
        width: #{bar_width}px;
        height: #{height-2}px;
        bottom: 2px;
        padding: 0 !important;
        margin: 0 !important;
        text-align: center;
        font-weight: bold;
        color: white;
        line-height: 1.5em;
      }
    HTML

    data.each_with_index do |d, index|
      bar_left = bar_offset + (bar_increment * index) 
      #      p "\n bar_left #{bar_left}"
      label_left = bar_left - 10
      background_offset = ( bar_image_offset * index ) 
      color = d[:color] || "#3e3e3e"
      html += <<-HTML
        #vertgraph-#{graph_id} dl dd.a#{index.to_s} { left: #{bar_left}px; background-color: #{color}; background-position: -#{background_offset}px bottom !important; }
        HTML
    end

    html += <<-"HTML"
      </style>
      <div id="vertgraph-#{graph_id}">
        <dl>
    HTML
    data.each_with_index do |d, index|
      scaled_value = scale_graph_value(d[:value], data_max, height-2)
      if (options[:display_value_on_bar])
        bar_text=(scaled_value < floor_cutoff ? '' : d[:value]).to_s  #text on top of the bar
      else
        bar_text=''
      end  

      @url = url_creator.call(index) if !url_creator.nil?
      html += <<-"HTML"
       <a href="#{@url}">
         <!-- Tooltip for bar group -->
         <dd class="a#{index.to_s}" style="height: #{height-2}px;background: none;" title="#{d[:title]}"></dd>
         <!-- Color bar -->
         <dd class="a#{index.to_s}" style="height: #{scaled_value}px;" title="#{d[:title]}">#{bar_text}</dd>
       </a>
     HTML
    end
    html += <<-"HTML"
        </dl>
      </div>
    HTML

    html
  end

  def scale_graph_value(data_value, data_max, max)
    ((data_value.to_f / data_max.to_f) * max).round
  end

  # We define our own list of countries because we'd like to have England,
  # Wales, Ireland and Scotland as options. United Kingdom is left as a choice
  # because that's how they play at the Olympic Games.
  def golaberto_options_for_country_select
    COUNTRIES
  end

  private
   COUNTRIES = [ "Afghanistan", "Albania", "Algeria", "American Samoa",
   "Andorra", "Angola", "Anguilla", "Antarctica", "Antigua And Barbuda",
   "Argentina", "Armenia", "Aruba", "Australia", "Austria", "Azerbaijan",
   "Bahamas", "Bahrain", "Bangladesh", "Barbados", "Belarus", "Belgium",
   "Belize", "Benin", "Bermuda", "Bhutan", "Bolivia", "Bosnia and Herzegowina",
   "Botswana", "Bouvet Island", "Brazil", "British Indian Ocean Territory",
   "Brunei Darussalam", "Bulgaria", "Burkina Faso", "Burma", "Burundi",
   "Cambodia", "Cameroon", "Canada", "Cape Verde", "Cayman Islands",
   "Central African Republic", "Chad", "Chile", "China", "Christmas Island",
   "Cocos (Keeling) Islands", "Colombia", "Comoros", "Congo",
   "Congo, the Democratic Republic of the", "Cook Islands", "Costa Rica",
   "Cote d'Ivoire", "Croatia", "Cuba", "Cyprus", "Czech Republic", "Denmark",
   "Djibouti", "Dominica", "Dominican Republic", "East Timor", "Ecuador",
   "Egypt", "El Salvador", "England", "Equatorial Guinea", "Eritrea", "Espana",
   "Estonia", "Ethiopia", "Falkland Islands", "Faroe Islands", "Fiji",
   "Finland", "France", "French Guiana", "French Polynesia",
   "French Southern Territories", "Gabon", "Gambia", "Georgia", "Germany",
   "Ghana", "Gibraltar", "Greece", "Greenland", "Grenada", "Guadeloupe",
   "Guam", "Guatemala", "Guinea", "Guinea-Bissau", "Guyana", "Haiti",
   "Heard and Mc Donald Islands", "Honduras", "Hong Kong", "Hungary",
   "Iceland", "India", "Indonesia", "Ireland", "Israel", "Italy", "Iran",
   "Iraq", "Jamaica", "Japan", "Jordan", "Kazakhstan", "Kenya", "Kiribati",
   "Korea, Republic of", "Korea (South)", "Kuwait", "Kyrgyzstan",
   "Lao People's Democratic Republic", "Latvia", "Lebanon", "Lesotho",
   "Liberia", "Liechtenstein", "Lithuania", "Luxembourg", "Macau", "Macedonia",
   "Madagascar", "Malawi", "Malaysia", "Maldives", "Mali", "Malta",
   "Marshall Islands", "Martinique", "Mauritania", "Mauritius", "Mayotte",
   "Mexico", "Micronesia, Federated States of", "Moldova, Republic of",
   "Monaco", "Mongolia", "Montserrat", "Morocco", "Mozambique", "Myanmar",
   "Namibia", "Nauru", "Nepal", "Netherlands", "Netherlands Antilles",
   "New Caledonia", "New Zealand", "Nicaragua", "Niger", "Nigeria", "Niue",
   "Norfolk Island", "Northern Ireland", "Northern Mariana Islands", "Norway",
   "Oman", "Pakistan", "Palau", "Panama", "Papua New Guinea", "Paraguay",
   "Peru", "Philippines", "Pitcairn", "Poland", "Portugal", "Puerto Rico",
   "Qatar", "Reunion", "Romania", "Russia", "Rwanda", "Saint Kitts and Nevis",
   "Saint Lucia", "Saint Vincent and the Grenadines", "Samoa (Independent)",
   "San Marino", "Sao Tome and Principe", "Saudi Arabia", "Scotland",
   "Senegal", "Serbia and Montenegro", "Seychelles", "Sierra Leone",
   "Singapore", "Slovakia", "Slovenia", "Solomon Islands", "Somalia",
   "South Africa", "South Georgia and the South Sandwich Islands",
   "South Korea", "Spain", "Sri Lanka", "St. Helena",
   "St. Pierre and Miquelon", "Suriname", "Svalbard and Jan Mayen Islands",
   "Swaziland", "Sweden", "Switzerland", "Taiwan", "Tajikistan", "Tanzania",
   "Thailand", "Togo", "Tokelau", "Tonga", "Trinidad", "Trinidad and Tobago",
   "Tunisia", "Turkey", "Turkmenistan", "Turks and Caicos Islands", "Tuvalu",
   "Uganda", "Ukraine", "United Arab Emirates", "United Kingdom",
   "United States", "United States Minor Outlying Islands", "Uruguay",
   "Uzbekistan", "Vanuatu", "Vatican City State (Holy See)", "Venezuela",
   "Viet Nam", "Virgin Islands (British)", "Virgin Islands (U.S.)", "Wales",
   "Wallis and Futuna Islands", "Western Sahara", "Yemen", "Zambia", "Zimbabwe"
   ] unless const_defined?("COUNTRIES")
end
