module OpenFlashChart

  class LineDot < LineBase
    def initialize args={}
      super
      @type = "line_dot"
    end
  end

  class DotValue < Base
    def initialize(value, args={})
      @value = value
      super args
    end
  end

end
