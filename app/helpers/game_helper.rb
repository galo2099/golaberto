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
    own_goal = goal && goal.own_goal == "1"
    penalty = goal && goal.penalty == "1"
    html << check_box_tag("#{home_away}_goal[#{i}][penalty]",
                          "1", penalty, :disabled => own_goal)
    html << hidden_field_tag("#{home_away}_goal[#{i}][penalty]", "0")
    html << "</td>"
    html << "<td>"
    html << check_box_tag("#{home_away}_goal[#{i}][own_goal]",
                          "1", own_goal,
                          :disabled => true)
    html << hidden_field_tag("#{home_away}_goal[#{i}][own_goal]", "0")
    html << "</td>"
    html << "</tr>"
  end
end
