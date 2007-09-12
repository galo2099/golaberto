module Riff

  module ClassMethods
    # Use this to set which attributes are compared.
    # The default is to use content_columns (set automatically when Riff is loaded).
    #
    # To diff attributes that aren't included by default:
    #
    #   diff :include => [array_of_attributes]
    #
    # To exclude some of the default attributes:
    #
    #   diff :exclude => [array_of_attributes]
    #
    # To specify the exact attributes to diff:
    #
    #   diff a, b, c, d
    #
    def diff(*attributes)
      if attributes.size == 1 && (attributes[0] == :default || attributes[0].is_a?(Hash))
        attributes = attributes.first
      end
      write_inheritable_attribute(:diff_options, attributes)
      write_inheritable_attribute(:diff_options_changed, true)
    end
    def diffable_attributes
      if read_inheritable_attribute(:diffable_attributes).nil? || read_inheritable_attribute(:diff_options_changed)
        case attributes = read_inheritable_attribute(:diff_options)
        when :default, Hash
          diffable_attributes = content_columns.map(&:name)
          if attributes.is_a?(Hash)
            if attributes.has_key?(:include)
              diffable_attributes = diffable_attributes + attributes[:include]
            elsif attributes.has_key?(:exclude)
              diffable_attributes = diffable_attributes - attributes[:exclude] 
            end
          end
        else
          diffable_attributes = attributes
        end
        write_inheritable_attribute :diffable_attributes, diffable_attributes
        write_inheritable_attribute :diff_options_changed, false
      end
      read_inheritable_attribute :diffable_attributes
    end
  end

  def self.included(base)
    base.extend ClassMethods
    base.diff :default
  end

  # Returns true if there are differences between self and model (or saved self); false otherwise.
  def diff?(model = self.class.find(id))
    self.class.diffable_attributes.each do |attribute|
      return true if send(attribute) != model.send(attribute)
    end
    return false
  end

  # Enumerates the differences between self and model (or saved self).
  def diff(model = self.class.find(id), &block)
    collect_differences(
      self.class.diffable_attributes.map { |a| [ a.to_sym, send(a), model.send(a) ] },
      &block
    )
  end

  # Enumerates the differences between self and hash.
  #
  # Uses the keys of the hash as the list attributes to compare,
  # instead of those given by self.class.diffable_columns.
  def diff_against_attributes(hash, &block)
    collect_differences(
      hash.inject([]) { |result, (a, v)| result << [ a.to_sym, send(a), v ] },
      &block
    )
  end

protected

  def collect_differences(attributes, &block)
    attributes.inject({}) do |difference, (attribute, a, b)|
      unless a == b
        if block
          block.call attribute, a, b
        end
        difference[attribute] = [a, b]
      end
      next difference
    end
  end
  
end
