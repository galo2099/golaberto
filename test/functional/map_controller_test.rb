require File.dirname(__FILE__) + '/../test_helper'
require 'map_controller'

# Re-raise errors caught by the controller.
class MapController; def rescue_action(e) raise e end; end

class MapControllerTest < ActionController::TestCase
  def setup
    @controller = MapController.new
    @request    = ActionController::TestRequest.new
    @response   = ActionController::TestResponse.new
  end

  def test_static_rejects_request_without_referrer
    get :static

    assert_response :forbidden
  end
end
