module GameHelper
  def input_for_goal(goal, i, home_away, aet)
    html = "<tr id='#{home_away}_#{aet}_goal_#{i}'>"
    html << "<td>"
    html << hidden_field_tag("#{home_away}_#{aet}_goal[#{i}][player_id]",
                             goal && goal.player_id)
    html << "<div id='#{home_away}_#{aet}_goal_player_#{i}'>"
    if goal
      html << goal.player.name if goal.player
    else
      html << "&nbsp;"
    end
    html << "</div></td>"
    html << drop_receiving_element("#{home_away}_#{aet}_goal_player_#{i}",
                                   :hoverclass => "drop_receiving",
                                   :onDrop => "receiveDrop")
    html << "<td>"
    html << text_field_tag("#{home_away}_#{aet}_goal[#{i}][time]",
                           goal && goal.time, :size => 2)
    html << "</td>"
    html << "<td>"
    own_goal = goal && goal.own_goal?
    penalty = goal && goal.penalty?
    html << hidden_field_tag("#{home_away}_#{aet}_goal[#{i}][penalty]", "0")
    html << check_box_tag("#{home_away}_#{aet}_goal[#{i}][penalty]",
                          "1", penalty, :disabled => own_goal)
    html << "</td>"
    html << "<td>"
    html << hidden_field_tag("#{home_away}_#{aet}_goal[#{i}][own_goal]", own_goal ? "1": "0")
    html << check_box_tag("#{home_away}_#{aet}_goal[#{i}][own_goal_check]",
                          "", own_goal,
                          :disabled => true)
    html << hidden_field_tag("#{home_away}_#{aet}_goal[#{i}][aet]", aet == "aet" ? "1" : "0")
    html << "</td>"
    html << "</tr>"
  end

  def game_versions(versions)
    diff_strings = []
    versions.inject(Game.new) do |old_game, new_game|
      str = content_tag(:h4, _("Version %{number}") % {:number => new_game.version})
      if new_game.updated_by
        str << content_tag(:small, content_tag(:div, content_tag(:a, image_tag("users/" + new_game.updated_by.small_logo, :class => "user-logo") + " " + new_game.updated_by.display_name, :href => url_for(:controller => :user, :action => :show, :id => new_game.updated_by)), :style => "overflow: hidden; white-space: nowrap; width: 100%"))
      end
      str << content_tag(:small, new_game.updated_at.strftime(_("%A, %d/%m/%Y at %H:%M")))
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

  def print_goals(goals, player_scored)
    count_home = 0
    count_away = 0
    html = ""
    goals.each do |goal|
      if goal.own_goal?
        if goal.team == @game.home
          home_goal = false
          away_goal = true
        else
          home_goal = true
          away_goal = false
        end
      else
        if goal.team == @game.home
          home_goal = true
          away_goal = false
        else
          home_goal = false
          away_goal = true
        end
      end
      player_scored[goal.player.id] = true if goal.player unless goal.own_goal?
      count_home += 1 if home_goal
      count_away += 1 if away_goal
      html << "<tr class='game_show_goals'>"
      html << "  <td class='game_show_home_score'>#{goal.player.name if home_goal}</td>"
      html << "  <td class='game_show_home_score'>"
      html << "    (pen)" if goal.penalty? and home_goal
      html << "    (own)" if goal.own_goal? and home_goal
      html << "  </td>"
      html << "  <td class='game_show_home_score'>"
      html << "    #{goal.time}'" if home_goal
      html << "  </td>"
      html << "  <td class='game_show_home_score'>"
      html << "    (#{count_home}-#{count_away})" if home_goal
      html << "  </td>"
      html << "  <td></td>"
      html << "  <td class='game_show_away_score'>"
      html << "    (#{count_home}-#{count_away})" if away_goal
      html << "  <td class='game_show_away_score'>"
      html << "    #{goal.time}'" if away_goal
      html << "  </td>"
      html << "  <td class='game_show_away_score'>"
      html << "    (pen)" if goal.penalty? and away_goal
      html << "    (own)" if goal.own_goal? and away_goal
      html << "  </td>"
      html << "  <td class='game_show_away_score'>"
      html << goal.player.name if away_goal
      html << "  </td>"
      html << "</tr>"
    end
    html
  end
end
