module GameHelper
  def input_for_goal(goal, i, home_away, aet)
    html = "<tr id='#{home_away}_#{aet}_goal_#{i}'>"
    html << "<td>"
    html << hidden_field_tag("#{home_away}_#{aet}_goal[#{i}][player_id]",
                             goal && goal.player_id)
    html << "<div id='#{home_away}_#{aet}_goal_player_#{i}'>"
    if goal
      html << h(goal.player.name) if goal.player
    else
      html << "&nbsp;"
    end
    html << "</div></td>"
    html << javascript_tag("$('##{home_away}_#{aet}_goal_player_#{i}').droppable({ hoverClass: 'drop_receiving', drop: receiveDrop });")
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
    html.html_safe
  end

  def game_versions(versions)
    diff_strings = []
    versions.inject(Game.new) do |old_game, new_game|
      str = content_tag(:h4, _("Version %{number}") % {:number => new_game.version})
      if new_game.updater
        str << content_tag(:small, content_tag(:div, content_tag(:a, image_tag(new_game.updater.avatar.url(:thumb), :class => "user-logo") + " " + new_game.updater.display_name, :href => url_for(:controller => :user, :action => :show, :id => new_game.updater)), :style => "overflow: hidden; white-space: nowrap; width: 100%"))
      end
      str << content_tag(:small, l(new_game.updated_at.in_time_zone(cookie_timezone)))
      str << content_tag(:br)
      str << formatted_diff(old_game.diff(new_game))
      diff_strings << str
      new_game
    end
    diff_strings.reverse.join("").html_safe
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
      html << "  <td class='game_show_home_score'>#{h(goal.player.name) if home_goal}</td>"
      html << "  <td class='game_show_home_score'>"
      html << "    (pen)" if goal.penalty? and home_goal
      html << "    (own)" if goal.own_goal? and home_goal
      html << "  </td>"
      html << "  <td class='game_show_home_score'>"
      html << "    #{h goal.time}'" if home_goal
      html << "  </td>"
      html << "  <td class='game_show_home_score'>"
      html << "  </td>"
      html << "  <td></td>"
      html << "  <td class='game_show_away_score'>"
      html << "  </td>"
      html << "  <td class='game_show_away_score'>"
      html << "    #{h goal.time}'" if away_goal
      html << "  </td>"
      html << "  <td class='game_show_away_score'>"
      html << "    (pen)" if goal.penalty? and away_goal
      html << "    (own)" if goal.own_goal? and away_goal
      html << "  </td>"
      html << "  <td class='game_show_away_score'>"
      html << h(goal.player.name) if away_goal
      html << "  </td>"
      html << "</tr>"
    end
    html.html_safe
  end
end
