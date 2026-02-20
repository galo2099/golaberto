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

# Rails 8 removed some APIs that legacy tests still use.
class ActiveModel::Errors
  def on(attribute)
    messages_for(attribute).first
  end unless method_defined?(:on)
end

class ActiveRecord::Base
  def update_attributes(attributes = {})
    update(attributes)
  end unless method_defined?(:update_attributes)

  def update_attributes!(attributes = {})
    update!(attributes)
  end unless method_defined?(:update_attributes!)
end

# Legacy functional tests call ActionController::TestRequest.new with no args.
if defined?(ActionController::TestRequest)
  class << ActionController::TestRequest
    alias_method :new_without_legacy_compat, :new unless method_defined?(:new_without_legacy_compat)

    def new(*args, **kwargs, &block)
      return create(ActionController::Base) if args.empty? && kwargs.empty?

      new_without_legacy_compat(*args, **kwargs, &block)
    end
  end
end

# Minimal Object#stub support for older tests.
unless Object.method_defined?(:stub)
  class Object
    def stub(name, value = nil, &block)
      metaclass = class << self; self; end
      had_method = respond_to?(name, true)
      previous_method = method(name) if had_method

      metaclass.send(:define_method, name) do |*args, **kwargs, &inner_block|
        if value.respond_to?(:call)
          value.call(*args, **kwargs, &inner_block)
        else
          value
        end
      end

      block.call
    ensure
      if had_method
        metaclass.send(:define_method, name, previous_method)
      else
        metaclass.send(:remove_method, name) rescue nil
      end
    end
  end
end
