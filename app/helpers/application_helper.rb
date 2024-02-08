# Methods added to this helper will be available to all templates in the application.
module ApplicationHelper
  include Pagy::Frontend

  def game_date(game)
    if game.has_time then game.date.in_time_zone(cookie_timezone) else game.date end
  end

  def formatted_date(game, day = false)
    unless game.date.nil?
      if day
        I18n.l game_date(game).to_date, :format => :date_weekday
      else
        I18n.l game_date(game).to_date, :format => :default
      end
    end
  end

  def formatted_time(game)
    if game.has_time
      I18n.l game.date.in_time_zone(cookie_timezone), :format => :hour_minute
    end
  end

  def javascript_include_jquery
    jquery_locale = I18n.locale
    ret = ""
    ret << stylesheet_link_tag("https://cdnjs.cloudflare.com/ajax/libs/jqueryui/1.13.2/themes/blitzer/jquery-ui.min.css")
    ret << javascript_include_tag("https://cdnjs.cloudflare.com/ajax/libs/jquery/3.7.1/jquery.min.js")
    ret << javascript_include_tag("https://cdnjs.cloudflare.com/ajax/libs/jqueryui/1.13.2/jquery-ui.min.js")
    ret << javascript_include_tag("https://ajax.googleapis.com/ajax/libs/jqueryui/1.11.4/i18n/jquery-ui-i18n.min.js")
    ret << javascript_tag("$(function(){$.datepicker.setDefaults($.datepicker.regional['#{jquery_locale}'])});")
    ret.html_safe
  end

  def javascript_include_flot
    ret = ""
    ret << javascript_include_tag("jquery.event.drag.js")
    ret << javascript_include_tag("https://cdn.jsdelivr.net/npm/flot@4.2.6/dist/es5/jquery.flot.min.js")
    ret.html_safe
  end

  def add_wbr_to_string(str, strings_to_break_after = [ '/' ])
    ret = String.new(str)
    ret = h(ret) unless str.html_safe?
    strings_to_break_after.each do |pattern|
      ret = ret.gsub(pattern) do |s|
        s + '<wbr />'
      end
    end
    ret.html_safe
  end

  def formatted_diff(diff)
    ret = ""
    diff.each do |key, value|
      name = Game.columns_hash[key.to_s] ? Game.human_attribute_name(key.to_s).downcase : key.to_s
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
            ret << h(goal.player.name) + " - "
            ret << h(goal.time.to_s) + " "
            ret << _("(penalty)") if goal.penalty?
            ret << _("(own_goal)") if goal.own_goal?
            ret << "<br>"
          end
        end
      when :stadium_id
        if value[0].nil?
          ret << sprintf(_("Added %s: %s<br>"), h(name), h(Stadium.find(value[1]).name)) rescue nil
        elsif value[1].nil?
          ret << sprintf(_("Removed %s: %s<br>"), h(name), h(Stadium.find(value[0]).name)) rescue nil
        else
          ret << sprintf(_("Changed %s: %s -> %s<br>"), h(name), h(Stadium.find(value[0]).name), h(Stadium.find(value[1]).name)) rescue nil
        end
      when :referee_id
        if value[0].nil?
          ret << sprintf(_("Added %s: %s<br>"), h(name), h(Referee.find(value[1]).name)) rescue nil
        elsif value[1].nil?
          ret << sprintf(_("Removed %s: %s<br>"), h(name), h(Referee.find(value[0]).name)) rescue nil
        else
          ret << sprintf(_("Changed %s: %s -> %s<br>"), h(name), h(Referee.find(value[0]).name), h(Referee.find(value[1]).name)) rescue nil
        end
      when :date
        if value[0].nil?
          ret << sprintf(_("Added %s: %s<br>"), h(name), h(value[1].in_time_zone(cookie_timezone)))
        elsif value[1].nil?
          ret << sprintf(_("Removed %s: %s<br>"), h(name), h(value[0].in_time_zone(cookie_timezone)))
        else
          ret << sprintf(_("Changed %s: %s -> %s<br>"), h(name), h(value[0].in_time_zone(cookie_timezone)), h(value[1].in_time_zone(cookie_timezone)))
        end
      when :home_field
        if value[0].nil?
          ret << sprintf(_("Added %s: %s<br>"), h(name), h(I18n.t("activerecord.attributes.game.home_field.#{value[1]}")))
        elsif value[1].nil?
          ret << sprintf(_("Removed %s: %s<br>"), h(name), h(I18n.t("activerecord.attributes.game.home_field.#{value[0]}")))
        else
          ret << sprintf(_("Changed %s: %s -> %s<br>"), h(name), h(I18n.t("activerecord.attributes.game.home_field.#{value[0]}")), h(I18n.t("activerecord.attributes.game.home_field.#{value[1]}")))
        end
      else
        if value[0].nil?
          ret << sprintf(_("Added %s: %s<br>"), h(name), h(value[1]))
        elsif value[1].nil?
          ret << sprintf(_("Removed %s: %s<br>"), h(name), h(value[0]))
        else
          ret << sprintf(_("Changed %s: %s -> %s<br>"), h(name), h(value[0]), h(value[1]))
        end
      end
    end
    ret.html_safe
  end

  class Country
    attr_reader :name
    attr_reader :alpha2

    CODE_POINTS = {
      'a' => '🇦',
      'b' => '🇧',
      'c' => '🇨',
      'd' => '🇩',
      'e' => '🇪',
      'f' => '🇫',
      'g' => '🇬',
      'h' => '🇭',
      'i' => '🇮',
      'j' => '🇯',
      'k' => '🇰',
      'l' => '🇱',
      'm' => '🇲',
      'n' => '🇳',
      'o' => '🇴',
      'p' => '🇵',
      'q' => '🇶',
      'r' => '🇷',
      's' => '🇸',
      't' => '🇹',
      'u' => '🇺',
      'v' => '🇻',
      'w' => '🇼',
      'x' => '🇽',
      'y' => '🇾',
      'z' => '🇿'
    }.freeze

    def initialize(name, alpha2)
      @name = name
      @alpha2 = alpha2
    end

    def emoji_flag
      case name
      when "England"
        "🏴󠁧󠁢󠁥󠁮󠁧󠁿"
      when "Northern Ireland"
        "🏴󠁧󠁢󠁮󠁩󠁲󠁿"
      when "Scotland"
        "🏴󠁧󠁢󠁳󠁣󠁴󠁿"
      when "Wales"
        "🏴󠁧󠁢󠁷󠁬󠁳󠁿"
      else
        alpha2.downcase.chars.map{|c| CODE_POINTS[c]}.join('')
      end
    end
  end

  class Continent
    cattr_reader :country_to_continent
    attr_reader :name
    attr_reader :countries

    def initialize(name, translation, countries)
      @name = name
      @countries = countries
      @@country_to_continent ||= {}
      countries.each{|c| @@country_to_continent[c.name] = self }
    end

    AFRICA = Continent.new("Africa", _("Africa"), [
      Country.new("Algeria", "DZ"),
      Country.new("Angola", "AO"),
      Country.new("Benin", "BJ"),
      Country.new("Botswana", "BW"),
      Country.new("Burkina Faso", "BF"),
      Country.new("Burundi", "BI"),
      Country.new("Cameroon", "CM"),
      Country.new("Cape Verde", "CV"),
      Country.new("Central African Republic", "CF"),
      Country.new("Chad", "TD"),
      Country.new("Comoros", "KM"),
      Country.new("Congo", "CG"),
      Country.new("DR Congo", "CD"),
      Country.new("Djibouti", "DJ"),
      Country.new("Egypt", "EG"),
      Country.new("Equatorial Guinea", "GQ"),
      Country.new("Eritrea", "ER"),
      Country.new("Ethiopia", "ET"),
      Country.new("Gabon", "GA"),
      Country.new("Gambia", "GM"),
      Country.new("Ghana", "GH"),
      Country.new("Guinea", "GN"),
      Country.new("Guinea-Bissau", "GW"),
      Country.new("Ivory Coast", "CI"),
      Country.new("Kenya", "KE"),
      Country.new("Lesotho", "LS"),
      Country.new("Liberia", "LR"),
      Country.new("Libya", "LY"),
      Country.new("Madagascar", "MG"),
      Country.new("Malawi", "MW"),
      Country.new("Mali", "ML"),
      Country.new("Mauritania", "MR"),
      Country.new("Mauritius", "MU"),
      Country.new("Morocco", "MA"),
      Country.new("Mozambique", "MZ"),
      Country.new("Namibia", "NA"),
      Country.new("Niger", "NE"),
      Country.new("Nigeria", "NG"),
      Country.new("Reunion", "RE"),
      Country.new("Rwanda", "RW"),
      Country.new("Sao Tome and Principe", "ST"),
      Country.new("Senegal", "SN"),
      Country.new("Seychelles", "SC"),
      Country.new("Sierra Leone", "SL"),
      Country.new("Somalia", "SO"),
      Country.new("South Africa", "ZA"),
      Country.new("South Sudan", "SS"),
      Country.new("Sudan", "SD"),
      Country.new("Swaziland", "SZ"),
      Country.new("Tanzania", "TZ"),
      Country.new("Togo", "TG"),
      Country.new("Tunisia", "TN"),
      Country.new("Uganda", "UG"),
      Country.new("Zambia", "ZM"),
      Country.new("Zimbabwe", "ZW"),
    ])

    ASIA = Continent.new("Asia", _("Asia"), [
      Country.new("Afghanistan", "AF"),
      Country.new("Australia", "AU"),
      Country.new("Bahrain", "BH"),
      Country.new("Bangladesh", "BD"),
      Country.new("Bhutan", "BT"),
      Country.new("Brunei", "BN"),
      Country.new("Cambodia", "KH"),
      Country.new("China", "CN"),
      Country.new("East Timor", "TL"),
      Country.new("Guam", "GU"),
      Country.new("Hong Kong", "HK"),
      Country.new("India", "IN"),
      Country.new("Iran", "IR"),
      Country.new("Indonesia", "ID"),
      Country.new("Iraq", "IQ"),
      Country.new("Japan", "JP"),
      Country.new("Jordan", "JO"),
      Country.new("Kuwait", "KW"),
      Country.new("Kyrgyzstan", "KG"),
      Country.new("Lao People's Democratic Republic", "LA"),
      Country.new("Lebanon", "LB"),
      Country.new("Macau", "MO"),
      Country.new("Malaysia", "MY"),
      Country.new("Maldives", "MV"),
      Country.new("Mongolia", "MN"),
      Country.new("Myanmar", "MM"),
      Country.new("Nepal", "NP"),
      Country.new("North Korea", "KP"),
      Country.new("Northern Mariana Islands", "MP"),
      Country.new("Oman", "OM"),
      Country.new("Pakistan", "PK"),
      Country.new("Palestine", "PS"),
      Country.new("Philippines", "PH"),
      Country.new("Qatar", "QA"),
      Country.new("Saudi Arabia", "SA"),
      Country.new("Singapore", "SG"),
      Country.new("South Korea", "KR"),
      Country.new("Sri Lanka", "LK"),
      Country.new("Syria", "SY"),
      Country.new("Taiwan", "TW"),
      Country.new("Tajikistan", "TJ"),
      Country.new("Thailand", "TH"),
      Country.new("Turkmenistan", "TM"),
      Country.new("United Arab Emirates", "AE"),
      Country.new("Uzbekistan", "UZ"),
      Country.new("Viet Nam", "VN"),
      Country.new("Yemen", "YE"),
    ])

    CONCACAF = Continent.new("North/Central America & Caribbean", _("North/Central America & Caribbean"), [
      Country.new("Anguilla", "AI"),
      Country.new("Antigua And Barbuda", "AG"),
      Country.new("Aruba", "AW"),
      Country.new("Bahamas", "BS"),
      Country.new("Barbados", "BB"),
      Country.new("Belize", "BZ"),
      Country.new("Bermuda", "BM"),
      Country.new("Canada", "CA"),
      Country.new("Cayman Islands", "KY"),
      Country.new("Costa Rica", "CR"),
      Country.new("Cuba", "CU"),
      Country.new("Curacao", "CW"),
      Country.new("Dominica", "DM"),
      Country.new("Dominican Republic", "DO"),
      Country.new("El Salvador", "SV"),
      Country.new("French Guiana", "GF"),
      Country.new("Grenada", "GD"),
      Country.new("Guadeloupe", "GP"),
      Country.new("Guatemala", "GT"),
      Country.new("Guyana", "GY"),
      Country.new("Haiti", "HT"),
      Country.new("Honduras", "HN"),
      Country.new("Jamaica", "JM"),
      Country.new("Martinique", "MQ"),
      Country.new("Mexico", "MX"),
      Country.new("Montserrat", "MS"),
      Country.new("Nicaragua", "NI"),
      Country.new("Panama", "PA"),
      Country.new("Puerto Rico", "PR"),
      Country.new("Saint Kitts and Nevis", "KN"),
      Country.new("Saint Lucia", "LC"),
      Country.new("Saint Martin", "MF"),
      Country.new("Saint Vincent and the Grenadines", "VC"),
      Country.new("Sint Maarten", "SX"),
      Country.new("Suriname", "SR"),
      Country.new("Trinidad and Tobago", "TT"),
      Country.new("Turks and Caicos Islands", "TC"),
      Country.new("United States", "US"),
      Country.new("Virgin Islands (British)", "VG"),
      Country.new("Virgin Islands (U.S.)", "VI"),
    ])

    EUROPE = Continent.new("Europe", _("Europe"), [
      Country.new("Albania", "AL"),
      Country.new("Andorra", "AD"),
      Country.new("Armenia", "AM"),
      Country.new("Austria", "AT"),
      Country.new("Azerbaijan", "AZ"),
      Country.new("Belarus", "BY"),
      Country.new("Belgium", "BE"),
      Country.new("Bosnia and Herzegovina", "BA"),
      Country.new("Bulgaria", "BG"),
      Country.new("Croatia", "HR"),
      Country.new("Cyprus", "CY"),
      Country.new("Czech Republic", "CZ"),
      Country.new("Denmark", "DK"),
      Country.new("England", ""),
      Country.new("Estonia", "EE"),
      Country.new("Faroe Islands", "FO"),
      Country.new("Finland", "FI"),
      Country.new("France", "FR"),
      Country.new("Georgia", "GE"),
      Country.new("Germany", "DE"),
      Country.new("Gibraltar", "GI"),
      Country.new("Greece", "GR"),
      Country.new("Hungary", "HU"),
      Country.new("Iceland", "IS"),
      Country.new("Ireland", "IE"),
      Country.new("Israel", "IL"),
      Country.new("Italy", "IT"),
      Country.new("Kazakhstan", "KZ"),
      Country.new("Kosovo", "XK"),
      Country.new("Latvia", "LV"),
      Country.new("Liechtenstein", "LI"),
      Country.new("Lithuania", "LT"),
      Country.new("Luxembourg", "LU"),
      Country.new("Macedonia", "MK"),
      Country.new("Malta", "MT"),
      Country.new("Moldova", "MD"),
      Country.new("Monaco", "MC"),
      Country.new("Montenegro", "ME"),
      Country.new("Netherlands", "NL"),
      Country.new("Northern Ireland", ""),
      Country.new("Norway", "NO"),
      Country.new("Poland", "PL"),
      Country.new("Portugal", "PT"),
      Country.new("Romania", "RO"),
      Country.new("Russia", "RU"),
      Country.new("San Marino", "SM"),
      Country.new("Scotland", ""),
      Country.new("Serbia", "RS"),
      Country.new("Serbia and Montenegro", "CS"),
      Country.new("Slovakia", "SK"),
      Country.new("Slovenia", "SI"),
      Country.new("Spain", "ES"),
      Country.new("Sweden", "SE"),
      Country.new("Switzerland", "CH"),
      Country.new("Turkey", "TR"),
      Country.new("Ukraine", "UA"),
      Country.new("United Kingdom", "GB"),
      Country.new("Wales", ""),
    ])

    OCEANIA = Continent.new("Oceania", _("Oceania"), [
      Country.new("American Samoa", "AS"),
      Country.new("Cook Islands", "CK"),
      Country.new("Fiji", "FJ"),
      Country.new("French Polynesia", "PF"),
      Country.new("Kiribati", "KI"),
      Country.new("New Caledonia", "NC"),
      Country.new("New Zealand", "NZ"),
      Country.new("Niue", "NU"),
      Country.new("Papua New Guinea", "PG"),
      Country.new("Samoa", "WS"),
      Country.new("Solomon Islands", "SB"),
      Country.new("Tonga", "TO"),
      Country.new("Tuvalu", "TV"),
      Country.new("Vanuatu", "VU"),
    ])

    SOUTH_AMERICA = Continent.new("South America", _("South America"), [
      Country.new("Argentina", "AR"),
      Country.new("Bolivia", "BO"),
      Country.new("Brazil", "BR"),
      Country.new("Chile", "CL"),
      Country.new("Colombia", "CO"),
      Country.new("Ecuador", "EC"),
      Country.new("Paraguay", "PY"),
      Country.new("Peru", "PE"),
      Country.new("Uruguay", "UY"),
      Country.new("Venezuela", "VE")
    ])

    ALL = {
      AFRICA.name => AFRICA,
      ASIA.name => ASIA,
      CONCACAF.name => CONCACAF,
      EUROPE.name => EUROPE,
      OCEANIA.name => OCEANIA,
      SOUTH_AMERICA.name => SOUTH_AMERICA,
    }

    def self.options_for_select
      ALL.map do |name, _|
        [_(name), name]
      end.sort{|a,b| ActiveSupport::Inflector.transliterate(a[0]) <=> ActiveSupport::Inflector.transliterate(b[0])}
    end
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
      [ _("Brunei"), "Brunei" ],
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
      [ _("Colombia"), "Colombia" ],
      [ _("Comoros"), "Comoros" ],
      [ _("Congo"), "Congo" ],
      [ _("DR Congo"), "DR Congo" ],
      [ _("Cook Islands"), "Cook Islands" ],
      [ _("Costa Rica"), "Costa Rica" ],
      [ _("Curacao"), "Curacao" ],
      [ _("Ivory Coast"), "Ivory Coast" ],
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
      [ _("Faroe Islands"), "Faroe Islands" ],
      [ _("Fiji"), "Fiji" ],
      [ _("Finland"), "Finland" ],
      [ _("France"), "France" ],
      [ _("French Guiana"), "French Guiana" ],
      [ _("French Polynesia"), "French Polynesia" ],
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
      [ _("Kosovo"), "Kosovo" ],
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
      [ _("Martinique"), "Martinique" ],
      [ _("Mauritania"), "Mauritania" ],
      [ _("Mauritius"), "Mauritius" ],
      [ _("Mexico"), "Mexico" ],
      [ _("Moldova"), "Moldova" ],
      [ _("Monaco"), "Monaco" ],
      [ _("Mongolia"), "Mongolia" ],
      [ _("Montenegro"), "Montenegro" ],
      [ _("Montserrat"), "Montserrat" ],
      [ _("Morocco"), "Morocco" ],
      [ _("Mozambique"), "Mozambique" ],
      [ _("Myanmar"), "Myanmar" ],
      [ _("Namibia"), "Namibia" ],
      [ _("Nepal"), "Nepal" ],
      [ _("Netherlands"), "Netherlands" ],
      [ _("New Caledonia"), "New Caledonia" ],
      [ _("New Zealand"), "New Zealand" ],
      [ _("Nicaragua"), "Nicaragua" ],
      [ _("Niger"), "Niger" ],
      [ _("Nigeria"), "Nigeria" ],
      [ _("Niue"), "Niue" ],
      [ _("North Korea"), "North Korea" ],
      [ _("Northern Ireland"), "Northern Ireland" ],
      [ _("Northern Mariana Islands"), "Northern Mariana Islands" ],
      [ _("Norway"), "Norway" ],
      [ _("Oman"), "Oman" ],
      [ _("Pakistan"), "Pakistan" ],
      [ _("Palestine"), "Palestine" ],
      [ _("Panama"), "Panama" ],
      [ _("Papua New Guinea"), "Papua New Guinea" ],
      [ _("Paraguay"), "Paraguay" ],
      [ _("Peru"), "Peru" ],
      [ _("Philippines"), "Philippines" ],
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
      [ _("Saint Martin"), "Saint Martin" ],
      [ _("Saint Vincent and the Grenadines"), "Saint Vincent and the Grenadines" ],
      [ _("Samoa"), "Samoa" ],
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
      [ _("Sint Maarten"), "Sint Maarten" ],
      [ _("Slovakia"), "Slovakia" ],
      [ _("Slovenia"), "Slovenia" ],
      [ _("Solomon Islands"), "Solomon Islands" ],
      [ _("Somalia"), "Somalia" ],
      [ _("South Africa"), "South Africa" ],
      [ _("South Korea"), "South Korea" ],
      [ _("South Sudan"), "South Sudan" ],
      [ _("Spain"), "Spain" ],
      [ _("Sri Lanka"), "Sri Lanka" ],
      [ _("Sudan"), "Sudan" ],
      [ _("Suriname"), "Suriname" ],
      [ _("Swaziland"), "Swaziland" ],
      [ _("Sweden"), "Sweden" ],
      [ _("Switzerland"), "Switzerland" ],
      [ _("Syria"), "Syria" ],
      [ _("Taiwan"), "Taiwan" ],
      [ _("Tajikistan"), "Tajikistan" ],
      [ _("Tanzania"), "Tanzania" ],
      [ _("Thailand"), "Thailand" ],
      [ _("Togo"), "Togo" ],
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
      [ _("Uruguay"), "Uruguay" ],
      [ _("Uzbekistan"), "Uzbekistan" ],
      [ _("Vanuatu"), "Vanuatu" ],
      [ _("Venezuela"), "Venezuela" ],
      [ _("Viet Nam"), "Viet Nam" ],
      [ _("Virgin Islands (British)"), "Virgin Islands (British)" ],
      [ _("Virgin Islands (U.S.)"), "Virgin Islands (U.S.)" ],
      [ _("Wales"), "Wales" ],
      [ _("Yemen"), "Yemen" ],
      [ _("Zambia"), "Zambia" ],
      [ _("Zimbabwe"), "Zimbabwe" ],
    ].sort{|a,b| ActiveSupport::Inflector.transliterate(a[0]) <=> ActiveSupport::Inflector.transliterate(b[0])}
  end

  def draggable_element(element_id, options = {})
    javascript_tag(draggable_element_js(element_id, options).chop!)
  end

  def draggable_element_js(element_id, options = {}) #:nodoc:
    %($("##{element_id}").draggable(#{options_for_javascript(options)});)
  end

  def options_for_javascript(options)
    if options.empty?
      '{}'
    else
      "{#{options.keys.map { |k| "#{k}:#{options[k]}" }.sort.join(', ')}}"
    end
  end

  def pagy_url_for(pagy, page, absolute: false, html_escaped: false)  # it was (page, pagy) in previous versions
    pagy_params = params.merge(pagy.vars[:page_param] => page, only_path: !absolute )
    pagy_params.permit!
    html_escaped ? url_for(pagy_params).gsub('&', '&amp;') : url_for(pagy_params)
  end

  def remote_function(options)
    function = ("$.ajax({url: '#{ url_for(options[:url]) }', type: '#{ options[:method] || 'GET' }', " +
    "data: #{ options[:with] ? options[:with] + "+ '&'" : '' } + " +
    "'authenticity_token=' + encodeURIComponent('#{ form_authenticity_token }')" +
    (options[:data_type] ? ", dataType: '" + options[:data_type] + "'" : "") +
    (options[:complete] ? ", complete: function(response) {" + options[:complete] + "}" : "") +
    (options[:success] ? ", success: function(response) {" + options[:success] + "}" : "") +
    (options[:failure] ? ", error: function(response) {" + options[:failure] + "}" : "") +
    (options[:before] ? ", beforeSend: function(data) {" + options[:before] + "}" : "") + "});")
    function = "#{function}; #{options[:after]}"  if options[:after]
    function = "if (#{options[:condition]}) { #{function}; }" if options[:condition]
    function = "if (confirm('#{escape_javascript(options[:confirm])}')) { #{function}; }" if options[:confirm]
    function.html_safe
  end

  def observe_field(id, options)
    if options[:with] && (options[:with] !~ /[\{=(.]/)
      options[:with] = "'#{options[:with]}=' + encodeURIComponent(value)"
    else
      options[:with] ||= 'value' unless options[:function]
    end

    callback = options[:function] || remote_function(options)
    javascript = <<JS
    $(document).ready(function() {
      $('##{id}').change(function() {
        var value = $(this).val();
        #{callback}
      });
    });
JS
    javascript_tag(javascript)
  end

  def submit_to_remote(name, value, options = {})
    options[:with] ||= '$(this.form).serialize()'

    html_options = options.delete(:html) || {}
    html_options[:name] = name

    button_to_remote(value, options, html_options)
  end

  def button_to_remote(name, options = {}, html_options = {})
    button_to_function(name, remote_function(options), html_options)
  end

  def button_to_function(name, *args)
    html_options = args.extract_options!.symbolize_keys

    function = args[0] || ''
    onclick = "#{"#{html_options[:onclick]}; " if html_options[:onclick]}#{function};"

    tag(:input, html_options.merge(:type => 'button', :value => name, :onclick => onclick.html_safe))
  end

  def link_to_remote(name, options = {}, html_options = {})
    link_to_function(name, remote_function(options), html_options)
  end

  def link_to_function(name, *args)
    html_options = args.extract_options!.symbolize_keys

    function = args[0] || ''
    onclick = "#{"#{html_options[:onclick]}; " if html_options[:onclick]}#{function}; return false;"
    href = html_options[:href] || '#'

    content_tag(:a, name, html_options.merge(:href => href, :onclick => onclick))
  end
end
