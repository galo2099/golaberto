{
  "en-US": {
    round: -> (_key, round:, **_options) {
      "#{round.ordinalize} Round"
    },
    "height": -> (_key, height:, **_options) {
      return "" if height.nil?
      total_inches = height / 2.54
      feet = total_inches / 12
      inches = total_inches % 12

      fractions = {
	'1/2' => '½',
	'1/3' => '⅓',
	'2/3' => '⅔',
	'1/4' => '¼',
	'3/4' => '¾',
	'1/5' => '⅕',
	'2/5' => '⅖',
	'3/5' => '⅗',
	'4/5' => '⅘',
	'1/6' => '⅙',
	'5/6' => '⅚',
	'1/7' => '⅐',
	'1/8' => '⅛',
	'3/8' => '⅜',
	'5/8' => '⅝',
	'7/8' => '⅞',
	'1/9' => '⅑',
	'1/10' => '⅒'
      }

      def round_to_nearest_fraction(float, denominator)
        # Calculate the nearest numerator
        nearest_numerator = (float * denominator).round

        # Find the GCD of the nearest numerator and the denominator
        gcd = nearest_numerator.gcd(denominator)

        # Simplify the fraction by dividing both the numerator and denominator by the GCD
        simplified_numerator = nearest_numerator / gcd
        simplified_denominator = denominator / gcd

        # Return the simplified fraction
        [simplified_numerator, simplified_denominator]
      end

      fraction = round_to_nearest_fraction(inches % 1, 8)
      if fraction == [1,1]
        inches += 1
        fraction = [0,1]
      end

      "#{feet.floor}′ #{inches.floor}#{"#{fractions["#{fraction[0]}/#{fraction[1]}"]}" if fraction[0] != 0}″"
    },
    "distance": -> (_key, distance:, **_options) {
      options = { precision: 0 }.merge(_options)
      "#{ApplicationController.helpers.number_with_precision(distance * 0.621371, precision: options[:precision])} miles"
    },
  }
}
