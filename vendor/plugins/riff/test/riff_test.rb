require File.join(File.dirname(__FILE__), 'test_setup')

class TestRiff < Test::Unit::TestCase

  def setup
    @alice, @bob, @eve = Riff::Person.find(:all)
  end

  def test_setup
    assert_equal 3, Riff::Person.count
    assert_equal @alice, Riff::Person.find(:first)
  end
  
  def test_boolean_diff
    assert @alice.diff?(@bob)
    assert @alice.diff?(@eve)
    assert ! @bob.diff?(@bob)
  end
  
  def test_diff_returning_hash
    expected_hash = { :name => ['bob', 'eve'] }
    assert_equal expected_hash, @bob.diff(@eve)
    expected_hash = { :name => ['alice', 'bob'], :age => [20, 21] }
    assert_equal expected_hash, @alice.diff(@bob)
  end

  def test_diff_against_saved_record
    assert ! @alice.diff?
    @alice.age = 21
    assert @alice.diff?
    expected_hash = { :age => [21, 20] }
    assert_equal expected_hash, @alice.diff
  end

  def test_diff_against_attribute_hash
    expected_hash = { :age => [21, 42] }
    assert_equal expected_hash, @bob.diff_against_attributes({ :age => 42 })
  end
  
  def test_diff_options
    default_attributes = ['name', 'age']
    assert_equal default_attributes, Person.diffable_attributes

    Person.diff *default_attributes
    assert_equal default_attributes, Person.diffable_attributes

    Person.diff :default
    assert_equal default_attributes, Person.diffable_attributes

    Person.diff 'default'
    assert_equal ['default'], Person.diffable_attributes

    Person.diff :age
    assert_equal [:age], Person.diffable_attributes
  end
  
  def test_diff_options_include
    Person.diff :include => ['id']
    assert_equal ['name', 'age', 'id'], Person.diffable_attributes
  end

  def test_diff_options_exclude
    Person.diff :exclude => ['age']
    assert_equal ['name'], Person.diffable_attributes
  end

end
