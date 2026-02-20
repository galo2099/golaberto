ENV["RAILS_ENV"] = "test"
require File.expand_path('../../config/environment', __FILE__)
require 'rails/test_help'
require Rails.root.join('lib/authenticated_test_helper')

class ActiveSupport::TestCase
  include AuthenticatedTestHelper

  # Setup all fixtures in test/fixtures/*.(yml|csv) for all tests in alphabetical order.
  #
  # Note: You'll currently still have to declare fixtures explicitly in integration tests
  # -- they do not yet inherit this setting
  fixtures :all

  # Add more helper methods to be used by all tests here...
end

# Legacy tests in this codebase still inherit from Test::Unit::TestCase.
# Map it to ActiveSupport::TestCase for compatibility with modern Minitest.
module Test
  module Unit
    TestCase = ActiveSupport::TestCase unless const_defined?(:TestCase)
  end
end
