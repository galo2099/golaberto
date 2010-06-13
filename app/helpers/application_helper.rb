# Methods added to this helper will be available to all templates in the application.
module ApplicationHelper
  def pagination_links_remote(paginator, ajax_options={}, page_options={}, html_options={})
    page_options = { :window_size => 8 }.merge(page_options)
    html = ""
    unless paginator.current.first?
      ajax_options[:url].merge!({:page => paginator.current.previous.number})
      html << link_to_remote(_("Prev"), ajax_options, html_options)
      html << " "
    else
      html << _("Prev ")
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
      html << _(" Next")
    end
    html
  end

  def formatted_diff(diff)
    ret = ""
    diff.each do |key, value|
      case key
      when :goals
        value.each do |hunk|
          hunk.each do |change|
            goal = Goal.new Hash[*change.element.flatten]
            case change.action
            when "-"
              ret << _("Removed goal ")
            when "+"
              ret << _("Added goal ")
            end
            ret << goal.player.name + " - "
            ret << goal.time.to_s + " "
            ret << _("(penalty)") if goal.penalty?
            ret << _("(own_goal)") if goal.own_goal?
            ret << "<br>"
          end
        end
      when :stadium_id
        if value[0].nil?
          ret << sprintf(_("Added stadium: %s<br>"), Stadium.find(value[1]).name) rescue nil
        elsif value[1].nil?
          ret << sprintf(_("Removed stadium: %s<br>"), Stadium.find(value[0]).name) rescue nil
        else
          ret << sprintf(_("Changed stadium: %s -> %s<br>"), Stadium.find(value[0]).name, Stadium.find(value[1]).name) rescue nil
        end
      when :referee_id
        if value[0].nil?
          ret << sprintf(_("Added referee: %s<br>"), Referee.find(value[1]).name) rescue nil
        elsif value[1].nil?
          ret << sprintf(_("Removed referee: %s<br>"), Referee.find(value[0]).name) rescue nil
        else
          ret << sprintf(_("Changed referee: %s -> %s<br>"), Referee.find(value[0]).name, Referee.find(value[1]).name) rescue nil
        end
      else
        name = Game.columns_hash[key.to_s] ? Game.columns_hash[key.to_s].human_name.downcase : key.to_s
        if value[0].nil?
          ret << sprintf(_("Added %s: %s<br>"), name, value[1])
        elsif value[1].nil?
          ret << sprintf(_("Removed %s: %s<br>"), name, value[0])
        else
          ret << sprintf(_("Changed %s: %s -> %s<br>"), name, value[0], value[1])
        end
      end
    end
    ret
  end

  # We define our own list of countries because we'd like to have England,
  # Wales, Ireland and Scotland as options. United Kingdom is left as a choice
  # because that's how they play at the Olympic Games.
  def golaberto_options_for_country_select
    [ [ _("Afghanistan"), "Afghanistan" ],
      [ _("Albania"), "Albania" ],
      [ _("Algeria"), "Algeria" ],
      [ _("American Samoa"), "American Samoa" ],
      [ _("Andorra"), "Andorra" ],
      [ _("Angola"), "Angola" ],
      [ _("Anguilla"), "Anguilla" ],
      [ _("Antigua And Barbuda"), "Antigua And Barbuda" ],
      [ _("Argentina"), "Argentina" ],
      [ _("Armenia"), "Armenia" ],
      [ _("Aruba"), "Aruba" ],
      [ _("Australia"), "Australia" ],
      [ _("Austria"), "Austria" ],
      [ _("Azerbaijan"), "Azerbaijan" ],
      [ _("Bahamas"), "Bahamas" ],
      [ _("Bahrain"), "Bahrain" ],
      [ _("Bangladesh"), "Bangladesh" ],
      [ _("Barbados"), "Barbados" ],
      [ _("Belarus"), "Belarus" ],
      [ _("Belgium"), "Belgium" ],
      [ _("Belize"), "Belize" ],
      [ _("Benin"), "Benin" ],
      [ _("Bermuda"), "Bermuda" ],
      [ _("Bhutan"), "Bhutan" ],
      [ _("Bolivia"), "Bolivia" ],
      [ _("Bosnia and Herzegovina"), "Bosnia and Herzegovina" ],
      [ _("Botswana"), "Botswana" ],
      [ _("Brazil"), "Brazil" ],
      [ _("British Indian Ocean Territory"), "British Indian Ocean Territory" ],
      [ _("Brunei Darussalam"), "Brunei Darussalam" ],
      [ _("Bulgaria"), "Bulgaria" ],
      [ _("Burkina Faso"), "Burkina Faso" ],
      [ _("Burundi"), "Burundi" ],
      [ _("Cambodia"), "Cambodia" ],
      [ _("Cameroon"), "Cameroon" ],
      [ _("Canada"), "Canada" ],
      [ _("Cape Verde"), "Cape Verde" ],
      [ _("Cayman Islands"), "Cayman Islands" ],
      [ _("Central African Republic"), "Central African Republic" ],
      [ _("Chad"), "Chad" ],
      [ _("Chile"), "Chile" ],
      [ _("China"), "China" ],
      [ _("Christmas Island"), "Christmas Island" ],
      [ _("Cocos (Keeling) Islands"), "Cocos (Keeling) Islands" ],
      [ _("Colombia"), "Colombia" ],
      [ _("Comoros"), "Comoros" ],
      [ _("Congo"), "Congo" ],
      [ _("Congo, the Democratic Republic of the"), "Congo, the Democratic Republic of the" ],
      [ _("Cook Islands"), "Cook Islands" ],
      [ _("Costa Rica"), "Costa Rica" ],
      [ _("Cote d'Ivoire"), "Cote d'Ivoire" ],
      [ _("Croatia"), "Croatia" ],
      [ _("Cuba"), "Cuba" ],
      [ _("Cyprus"), "Cyprus" ],
      [ _("Czech Republic"), "Czech Republic" ],
      [ _("Denmark"), "Denmark" ],
      [ _("Djibouti"), "Djibouti" ],
      [ _("Dominica"), "Dominica" ],
      [ _("Dominican Republic"), "Dominican Republic" ],
      [ _("East Timor"), "East Timor" ],
      [ _("Ecuador"), "Ecuador" ],
      [ _("Egypt"), "Egypt" ],
      [ _("El Salvador"), "El Salvador" ],
      [ _("England"), "England" ],
      [ _("Equatorial Guinea"), "Equatorial Guinea" ],
      [ _("Eritrea"), "Eritrea" ],
      [ _("Estonia"), "Estonia" ],
      [ _("Ethiopia"), "Ethiopia" ],
      [ _("Falkland Islands"), "Falkland Islands" ],
      [ _("Faroe Islands"), "Faroe Islands" ],
      [ _("Fiji"), "Fiji" ],
      [ _("Finland"), "Finland" ],
      [ _("France"), "France" ],
      [ _("French Guiana"), "French Guiana" ],
      [ _("French Polynesia"), "French Polynesia" ],
      [ _("French Southern Territories"), "French Southern Territories" ],
      [ _("Gabon"), "Gabon" ],
      [ _("Gambia"), "Gambia" ],
      [ _("Georgia"), "Georgia" ],
      [ _("Germany"), "Germany" ],
      [ _("Ghana"), "Ghana" ],
      [ _("Gibraltar"), "Gibraltar" ],
      [ _("Greece"), "Greece" ],
      [ _("Greenland"), "Greenland" ],
      [ _("Grenada"), "Grenada" ],
      [ _("Guadeloupe"), "Guadeloupe" ],
      [ _("Guam"), "Guam" ],
      [ _("Guatemala"), "Guatemala" ],
      [ _("Guinea"), "Guinea" ],
      [ _("Guinea-Bissau"), "Guinea-Bissau" ],
      [ _("Guyana"), "Guyana" ],
      [ _("Haiti"), "Haiti" ],
      [ _("Heard and Mc Donald Islands"), "Heard and Mc Donald Islands" ],
      [ _("Honduras"), "Honduras" ],
      [ _("Hong Kong"), "Hong Kong" ],
      [ _("Hungary"), "Hungary" ],
      [ _("Iceland"), "Iceland" ],
      [ _("India"), "India" ],
      [ _("Indonesia"), "Indonesia" ],
      [ _("Ireland"), "Ireland" ],
      [ _("Israel"), "Israel" ],
      [ _("Italy"), "Italy" ],
      [ _("Iran"), "Iran" ],
      [ _("Iraq"), "Iraq" ],
      [ _("Jamaica"), "Jamaica" ],
      [ _("Japan"), "Japan" ],
      [ _("Jordan"), "Jordan" ],
      [ _("Kazakhstan"), "Kazakhstan" ],
      [ _("Kenya"), "Kenya" ],
      [ _("Kiribati"), "Kiribati" ],
      [ _("Kuwait"), "Kuwait" ],
      [ _("Kyrgyzstan"), "Kyrgyzstan" ],
      [ _("Lao People's Democratic Republic"), "Lao People's Democratic Republic" ],
      [ _("Latvia"), "Latvia" ],
      [ _("Lebanon"), "Lebanon" ],
      [ _("Lesotho"), "Lesotho" ],
      [ _("Liberia"), "Liberia" ],
      [ _("Libya"), "Libya" ],
      [ _("Liechtenstein"), "Liechtenstein" ],
      [ _("Lithuania"), "Lithuania" ],
      [ _("Luxembourg"), "Luxembourg" ],
      [ _("Macau"), "Macau" ],
      [ _("Macedonia"), "Macedonia" ],
      [ _("Madagascar"), "Madagascar" ],
      [ _("Malawi"), "Malawi" ],
      [ _("Malaysia"), "Malaysia" ],
      [ _("Maldives"), "Maldives" ],
      [ _("Mali"), "Mali" ],
      [ _("Malta"), "Malta" ],
      [ _("Marshall Islands"), "Marshall Islands" ],
      [ _("Martinique"), "Martinique" ],
      [ _("Mauritania"), "Mauritania" ],
      [ _("Mauritius"), "Mauritius" ],
      [ _("Mayotte"), "Mayotte" ],
      [ _("Mexico"), "Mexico" ],
      [ _("Micronesia, Federated States of"), "Micronesia, Federated States of" ],
      [ _("Moldova, Republic of"), "Moldova, Republic of" ],
      [ _("Monaco"), "Monaco" ],
      [ _("Mongolia"), "Mongolia" ],
      [ _("Montenegro"), "Montenegro" ],
      [ _("Montserrat"), "Montserrat" ],
      [ _("Morocco"), "Morocco" ],
      [ _("Mozambique"), "Mozambique" ],
      [ _("Myanmar"), "Myanmar" ],
      [ _("Namibia"), "Namibia" ],
      [ _("Nauru"), "Nauru" ],
      [ _("Nepal"), "Nepal" ],
      [ _("Netherlands"), "Netherlands" ],
      [ _("Netherlands Antilles"), "Netherlands Antilles" ],
      [ _("New Caledonia"), "New Caledonia" ],
      [ _("New Zealand"), "New Zealand" ],
      [ _("Nicaragua"), "Nicaragua" ],
      [ _("Niger"), "Niger" ],
      [ _("Nigeria"), "Nigeria" ],
      [ _("Niue"), "Niue" ],
      [ _("Norfolk Island"), "Norfolk Island" ],
      [ _("North Korea"), "North Korea" ],
      [ _("Northern Ireland"), "Northern Ireland" ],
      [ _("Northern Mariana Islands"), "Northern Mariana Islands" ],
      [ _("Norway"), "Norway" ],
      [ _("Oman"), "Oman" ],
      [ _("Pakistan"), "Pakistan" ],
      [ _("Palau"), "Palau" ],
      [ _("Palestine"), "Palestine" ],
      [ _("Panama"), "Panama" ],
      [ _("Papua New Guinea"), "Papua New Guinea" ],
      [ _("Paraguay"), "Paraguay" ],
      [ _("Peru"), "Peru" ],
      [ _("Philippines"), "Philippines" ],
      [ _("Pitcairn"), "Pitcairn" ],
      [ _("Poland"), "Poland" ],
      [ _("Portugal"), "Portugal" ],
      [ _("Puerto Rico"), "Puerto Rico" ],
      [ _("Qatar"), "Qatar" ],
      [ _("Reunion"), "Reunion" ],
      [ _("Romania"), "Romania" ],
      [ _("Russia"), "Russia" ],
      [ _("Rwanda"), "Rwanda" ],
      [ _("Saint Kitts and Nevis"), "Saint Kitts and Nevis" ],
      [ _("Saint Lucia"), "Saint Lucia" ],
      [ _("Saint Vincent and the Grenadines"), "Saint Vincent and the Grenadines" ],
      [ _("Samoa (Independent)"), "Samoa (Independent)" ],
      [ _("San Marino"), "San Marino" ],
      [ _("Sao Tome and Principe"), "Sao Tome and Principe" ],
      [ _("Saudi Arabia"), "Saudi Arabia" ],
      [ _("Scotland"), "Scotland" ],
      [ _("Senegal"), "Senegal" ],
      [ _("Serbia"), "Serbia" ],
      [ _("Serbia and Montenegro"), "Serbia and Montenegro" ],
      [ _("Seychelles"), "Seychelles" ],
      [ _("Sierra Leone"), "Sierra Leone" ],
      [ _("Singapore"), "Singapore" ],
      [ _("Slovakia"), "Slovakia" ],
      [ _("Slovenia"), "Slovenia" ],
      [ _("Solomon Islands"), "Solomon Islands" ],
      [ _("Somalia"), "Somalia" ],
      [ _("South Africa"), "South Africa" ],
      [ _("South Georgia and the South Sandwich Islands"), "South Georgia and the South Sandwich Islands" ],
      [ _("South Korea"), "South Korea" ],
      [ _("Spain"), "Spain" ],
      [ _("Sri Lanka"), "Sri Lanka" ],
      [ _("St. Helena"), "St. Helena" ],
      [ _("St. Pierre and Miquelon"), "St. Pierre and Miquelon" ],
      [ _("Sudan"), "Sudan" ],
      [ _("Suriname"), "Suriname" ],
      [ _("Svalbard and Jan Mayen Islands"), "Svalbard and Jan Mayen Islands" ],
      [ _("Swaziland"), "Swaziland" ],
      [ _("Sweden"), "Sweden" ],
      [ _("Switzerland"), "Switzerland" ],
      [ _("Syria"), "Syria" ],
      [ _("Taiwan"), "Taiwan" ],
      [ _("Tajikistan"), "Tajikistan" ],
      [ _("Tanzania"), "Tanzania" ],
      [ _("Thailand"), "Thailand" ],
      [ _("Togo"), "Togo" ],
      [ _("Tokelau"), "Tokelau" ],
      [ _("Tonga"), "Tonga" ],
      [ _("Trinidad and Tobago"), "Trinidad and Tobago" ],
      [ _("Tunisia"), "Tunisia" ],
      [ _("Turkey"), "Turkey" ],
      [ _("Turkmenistan"), "Turkmenistan" ],
      [ _("Turks and Caicos Islands"), "Turks and Caicos Islands" ],
      [ _("Tuvalu"), "Tuvalu" ],
      [ _("Uganda"), "Uganda" ],
      [ _("Ukraine"), "Ukraine" ],
      [ _("United Arab Emirates"), "United Arab Emirates" ],
      [ _("United Kingdom"), "United Kingdom" ],
      [ _("United States"), "United States" ],
      [ _("United States Minor Outlying Islands"), "United States Minor Outlying Islands" ],
      [ _("Uruguay"), "Uruguay" ],
      [ _("Uzbekistan"), "Uzbekistan" ],
      [ _("Vanuatu"), "Vanuatu" ],
      [ _("Vatican City State (Holy See)"), "Vatican City State (Holy See)" ],
      [ _("Venezuela"), "Venezuela" ],
      [ _("Viet Nam"), "Viet Nam" ],
      [ _("Virgin Islands (British)"), "Virgin Islands (British)" ],
      [ _("Virgin Islands (U.S.)"), "Virgin Islands (U.S.)" ],
      [ _("Wales"), "Wales" ],
      [ _("Wallis and Futuna Islands"), "Wallis and Futuna Islands" ],
      [ _("Western Sahara"), "Western Sahara" ],
      [ _("Yemen"), "Yemen" ],
      [ _("Zambia"), "Zambia" ],
      [ _("Zimbabwe"), "Zimbabwe" ],
    ].sort{|a,b| ActiveSupport::Inflector.transliterate(a[0]) <=> ActiveSupport::Inflector.transliterate(b[0])}
  end

end
