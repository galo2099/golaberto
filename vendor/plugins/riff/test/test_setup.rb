$:.unshift File.join(File.dirname(__FILE__), '..', '..', '..', '..')

require 'config/environment'

ActiveRecord::Base.establish_connection('adapter' => 'sqlite3', 'database' => ':memory:')

ActiveRecord::Schema.define do
  create_table :people do |t|
    t.column :name, :string
    t.column :age, :integer
  end
end

module Riff
  class Person < ActiveRecord::Base
  end
end

Riff::Person.create :name => 'alice', :age => 20
Riff::Person.create :name => 'bob', :age => 21
Riff::Person.create :name => 'eve', :age => 21
