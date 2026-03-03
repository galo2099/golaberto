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

  def test_static_rejects_request_with_mismatched_referrer
    @request.env['HTTP_REFERER'] = 'http://test.host/team/show/1'

    @request.env['QUERY_STRING'] = 'referrer=http%3A%2F%2Ftest.host%2Fteam%2Fshow%2F2'
    get :static

    assert_response :forbidden
  end

  def test_static_accepts_request_with_matching_referrer
    @request.env['HTTP_REFERER'] = 'http://test.host/team/show/1'
    fake_response = Net::HTTPOK.new('1.1', '200', 'OK')
    fake_response['content-type'] = 'image/jpeg'
    fake_response.instance_variable_set(:@read, true)
    fake_response.body = 'jpg'

    fake_credentials = Struct.new(:google_api).new({ secret_sign: 'test-sign' })

    Rails.application.stub(:credentials, fake_credentials) do
      GoogleUrlSigner.stub(:sign, 'https://maps.googleapis.com/maps/api/staticmap?key=test') do
        Net::HTTP.stub(:start, fake_response) do
          @request.env['QUERY_STRING'] = 'referrer=http%3A%2F%2Ftest.host%2Fteam%2Fshow%2F1'
          get :static
        end
      end
    end

    assert_response :success
  end
end
