require File.dirname(__FILE__) + '/../test_helper'
require 'stadium_controller'

# Re-raise errors caught by the controller.
class StadiumController; def rescue_action(e) raise e end; end

class StadiumControllerTest < Test::Unit::TestCase
  def setup
    @controller = StadiumController.new
    @request    = ActionController::TestRequest.new
    @response   = ActionController::TestResponse.new
  end

  # Replace this with your real tests.
  def test_truth
    assert true
  end
end
