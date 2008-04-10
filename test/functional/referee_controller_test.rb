require File.dirname(__FILE__) + '/../test_helper'
require 'referee_controller'

# Re-raise errors caught by the controller.
class RefereeController; def rescue_action(e) raise e end; end

class RefereeControllerTest < Test::Unit::TestCase
  def setup
    @controller = RefereeController.new
    @request    = ActionController::TestRequest.new
    @response   = ActionController::TestResponse.new
  end

  # Replace this with your real tests.
  def test_truth
    assert true
  end
end
