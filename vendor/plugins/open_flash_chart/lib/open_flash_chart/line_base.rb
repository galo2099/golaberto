module OpenFlashChart

  class LineBase < Base
    def initialize args={}
      super
      @type = "line_dot"
    end

    def loop
      @loop = true
    end
  end

end
