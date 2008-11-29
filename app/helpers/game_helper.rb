module GameHelper
  def input_for_goal(goal, i, home_away)
    html = "<tr id='#{home_away}_goal_#{i}'>"
    html << "<td>"
    html << hidden_field_tag("#{home_away}_goal[#{i}][player_id]",
                             goal && goal.player_id)
    html << "<div id='#{home_away}_goal_player_#{i}'>"
    if goal
      html << goal.player.name if goal.player
    else
      html << "&nbsp;"
    end
    html << "</div></td>"
    html << drop_receiving_element("#{home_away}_goal_player_#{i}",
                                   :hoverclass => "drop_receiving",
                                   :onDrop => "receiveDrop")
    html << "<td>"
    html << text_field_tag("#{home_away}_goal[#{i}][time]",
                           goal && goal.time, :size => 2)
    html << "</td>"
    html << "<td>"
    own_goal = goal && goal.own_goal?
    penalty = goal && goal.penalty?
    html << check_box_tag("#{home_away}_goal[#{i}][penalty]",
                          "1", penalty, :disabled => own_goal)
    html << hidden_field_tag("#{home_away}_goal[#{i}][penalty]", "0")
    html << "</td>"
    html << "<td>"
    html << check_box_tag("#{home_away}_goal[#{i}][own_goal_check]",
                          "", own_goal,
                          :disabled => true)
    html << hidden_field_tag("#{home_away}_goal[#{i}][own_goal]", own_goal ? "1": "0")
    html << "</td>"
    html << "</tr>"
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
          ret << _("Added stadium: #{Stadium.find(value[1]).name}<br>")
        elsif value[1].nil?
          ret << _("Removed stadium: #{Stadium.find(value[0]).name}<br>")
        else
          ret << _("Changed stadium: #{Stadium.find(value[0]).name} -> #{Stadium.find(value[1]).name}<br>")
        end
      when :referee_id
        if value[0].nil?
          ret << _("Added referee: #{Referee.find(value[1]).name}<br>")
        elsif value[1].nil?
          ret << _("Removed referee: #{Referee.find(value[0]).name}<br>")
        else
          ret << _("Changed referee: #{Referee.find(value[0]).name} -> #{Referee.find(value[1]).name}<br>")
        end
      else
        if value[0].nil?
          ret << _("Added #{key}: #{value[1]}<br>")
        elsif value[1].nil?
          ret << _("Removed #{key}: #{value[0]}<br>")
        else
          ret << _("Changed #{key}: #{value[0]} -> #{value[1]}<br>")
        end
      end
    end
    ret
  end
end
