module PlayerHelper
  def position_to_string(position, format = :short)
    if format == :short
      case position
      when "g"
        s_("position|g")
      when "dr"
        s_("position|dr")
      when "dc"
        s_("position|dc")
      when "dl"
        s_("position|dl")
      when "dm"
        s_("position|dm")
      when "cm"
        s_("position|cm")
      when "am"
        s_("position|am")
      when "fw"
        s_("position|fw")
      end
    else
     case position
      when "g"
        s_("position|goalkeeper")
      when "dr"
        s_("position|right defender")
      when "dc"
        s_("position|central defender")
      when "dl"
        s_("position|left defender")
      when "dm"
        s_("position|defensive midfielder")
      when "cm"
        s_("position|center midfielder")
      when "am"
        s_("position|attacking midfielder")
      when "fw"
        s_("position|forward")
      end
    end
  end
end
