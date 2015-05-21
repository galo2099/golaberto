require 'fuzzy/jarowinkler'

class FuzzyTeamMatch
  DEBUG = false

  # Inspired by http://seatgeek.com/blog/dev/fuzzywuzzy-fuzzy-string-matching-in-python
  def getDistance(a1, a2)
    @distance ||= {}
    @distance[[a1, a2]] ||= (
      a1 = ActiveSupport::Inflector.transliterate(a1).downcase
      a2 = ActiveSupport::Inflector.transliterate(a2).downcase
      a1_tokens = a1.scan(/[[:alnum:]']+/).to_set
      a2_tokens = a2.scan(/[[:alnum:]']+/).to_set
      t0 = (a1_tokens & a2_tokens).sort
      t1 = t0 + (a1_tokens - a2_tokens).to_a.sort
      t2 = t0 + (a2_tokens - a1_tokens).to_a.sort
      [ token_diff(t0, t1) / t1.size, token_diff(t0, t2) / t2.size, token_diff(t1, t2) ].sum
    )
  end

  def token_diff(t0, t1)
    return 0 if t0.empty? or t1.empty?
    if t0.size > t1.size
      t0, t1 = t1, t0
    end
    j = JaroWinklerPure.new
    token_scores = t0.map{|a| [a].product(t1).tap{|t| p t if DEBUG }.
        # Get the JaroWinkler score between the string and scale it to the string size 
        map{|a,b|j.getDistance(a,b) * [ a.size, b.size ].min } }
    p token_scores if DEBUG

    t1.size.times.to_a.permutation(t0.size).map{|x| x.zip(token_scores).map{|i,s| s[i]}.sum }.max
  end
end
