class Poisson
  class InvalidMean < Exception; end;

  attr_accessor :mean

  # Instantiate a new Poisson instance, passing the mean number of occurrence of the event.
  def initialize(mean = 0)
    if !mean.is_a?(Numeric)
      raise InvalidMean, "the mean must be numeric"
    elsif mean < 0
      raise InvalidMean, "the mean must be positive"
    end

    @probability = Hash.new{|h,k| h[k] = Hash.new}
    @mean = mean
  end

  def probability(actual)
    @probability[@mean][actual] ||= @mean ** actual * Math.exp(-@mean) / Poisson.factorial(actual)
  end

  alias_method :p, :probability

  def find_mean_from(distribution)
    nn = distribution.inject(0){|sum,x|sum+x}

    cs = 1*10000000000000000
    @mean = 0;
    sump = 0;
    rawp = Array.new(distribution.size)
    exp = Array.new(distribution.size)

    15000.times do |jjz|
      csnew = 0;
      @mean += 0.01;
      sump = 0;

      distribution.size.times do |j|
        rawp[j] = Math.exp(-@mean) * (@mean ** j) / Poisson.factorial(j);
      end

      distribution.size.times do |j|
        exp[j] = rawp[j] * nn;
        csnew += (distribution[j] - exp[j]) ** 2
      end
      if csnew < cs then
        cs = csnew
      else
        @mean -= 0.01
        break
      end
    end
    self
  end

  def rand
    die = Kernel.rand
    i = 0
    accumlated_probability = p(i)
    while die > accumlated_probability
      i += 1
      accumlated_probability += p(i)
    end
    i
  end

  def self.factorial(n)
    r = 1
    (2..n).each do |i|
      r *= i
    end
    r
  end
end
