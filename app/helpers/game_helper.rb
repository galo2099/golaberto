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

  def game_versions(versions)
    diff_strings = []
    versions.inject(Game.new) do |old_game, new_game|
      str = content_tag(:h4, _("Version %{number}") % {:number => new_game.version})
      if new_game.updated_by
        str << content_tag(:div, image_tag("users/" + new_game.updated_by.small_logo, :border => 1) + " " + new_game.updated_by.display_login,
                           :style => "overflow: hidden; white-space: nowrap; width: 100%")
      end
      str << new_game.updated_at.strftime(_("%A, %d/%m/%Y at %H:%M"))
      str << content_tag(:br)
      str << formatted_diff(old_game.diff(new_game))
      diff_strings << str
      new_game
    end
    diff_strings.reverse.join ""
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
          ret << sprintf(_("Added stadium: %s<br>"), Stadium.find(value[1]).name)
        elsif value[1].nil?
          ret << sprintf(_("Removed stadium: %s<br>"), Stadium.find(value[0]).name)
        else
          ret << sprintf(_("Changed stadium: %s -> %s<br>"), Stadium.find(value[0]).name, Stadium.find(value[1]).name)
        end
      when :referee_id
        if value[0].nil?
          ret << sprintf(_("Added referee: %s<br>"), Referee.find(value[1]).name)
        elsif value[1].nil?
          ret << sprintf(_("Removed referee: %s<br>"), Referee.find(value[0]).name)
        else
          ret << sprintf(_("Changed referee: %s -> %s<br>"), Referee.find(value[0]).name, Referee.find(value[1]).name)
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
end
