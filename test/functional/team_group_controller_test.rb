require File.dirname(__FILE__) + '/../test_helper'
require 'team_group_controller'

# Re-raise errors caught by the controller.
class TeamGroupController; def rescue_action(e) raise e end; end

class TeamGroupControllerTest < Test::Unit::TestCase
  def setup
    @controller = TeamGroupController.new
    @request    = ActionController::TestRequest.new
    @response   = ActionController::TestResponse.new
  end

  # Replace this with your real tests.
  def test_truth
    assert true
  end
end
