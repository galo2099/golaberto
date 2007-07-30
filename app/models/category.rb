class Category < ActiveRecord::Base
  DEFAULT_CATEGORY = 1

  validates_presence_of :name
end
