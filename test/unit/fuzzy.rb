# encoding: utf-8

require File.dirname(__FILE__) + '/../test_helper'
require 'fuzzy/fuzzy'

class FuzzyTeamMatchTest < Test::Unit::TestCase
  def test_right_ordering
    german = ["Bayern Munich", "Werder Bremen", "Schalke 04", "Wolfsburg", "Hamburger", "Stuttgart", "Bayer Leverkusen", "Hannover 96", "Hertha Berlin", "Borussia Mönchengladbach", "Borussia Dortmund", "Hoffenheim", "1. FC Köln", "Freiburg", "Mainz 05", "Nürnberg", "Kaiserslautern", "Augsburg"]
    german.each do |team|
      assert_equal team, find_best_match(german, team)
    end
    assert_equal "Nürnberg", find_best_match(german, "1. FC Nuremberg")
    assert_equal "Borussia Mönchengladbach", find_best_match(german, "M'gladbach")

    brazil = ["Palmeiras-SP", "Coruripe-AL", "América-RN", "Horizonte-CE", "Ceará-CE", "Gama-DF", "Paraná-PR", "Luverdense-MT", "Cruzeiro-MG", "Rio Branco-AC", "Chapecoense-SC", "São Mateus-ES", "Atlético-PR", "Sampaio Corrêa-MA", "Criciúma-SC", "Madureira-RJ", "Grêmio-RS", "River Plate-SE", "Ipatinga-MG", "Real Noroeste-ES", "Náutico-PE", "Santa Cruz-RN", "Fortaleza-CE", "Comercial-PI", "Bahia-BA", "Auto Esporte-PB", "Remo-PA", "Real-RR", "Portuguesa-SP", "Cuiabá-MT", "Juventude-RS", "Operário-PR", "São Paulo-SP", "Independente-PA", "Bahia de Feira-BA", "Aquidauanense-MS", "Atlético-GO", "Gurupi-TO", "Ponte Preta-SP", "Sapucaiense-RS", "Atlético-MG", "CENE-MS", "Santa Cruz-PE", "Penarol-AM", "América-MG", "Boavista-RJ", "Goiás-GO", "Paulista-SP", "Coritiba-PR", "Nacional-AM", "ASA-AL", "Santa Quitéria-MA", "Sport-PE", "4 de Julho-PI", "Paysandu-PA", "Espigão-RO", "Botafogo-RJ", "Treze-PB", "Brasiliense-DF", "Guarani-SP", "Vitória-BA", "São Domingos-SE", "ABC-RN", "Trem-AP"]
    brazil.each do |team|
      assert_equal team, find_best_match(brazil, team)
    end

    england = ["Arsenal", "Aston Villa", "Blackburn Rovers", "Bolton Wanderers", "Chelsea", "Everton", "Fulham", "Liverpool", "Manchester City", "Manchester United", "Newcastle United", "Norwich City", "Tottenham Hotspur", "West Bromwich Albion", "Sunderland", "Wigan Athletic", "Stoke City", "Wolverhampton", "Queens Park Rangers", "Swansea City"]
    england.each do |team|
      assert_equal team, find_best_match(england, team)
    end
    assert_equal "Blackburn Rovers", find_best_match(england, "Blackburn")
    assert_equal "Bolton Wanderers", find_best_match(england, "Bolton")
    assert_equal "Manchester City", find_best_match(england, "Man City")
    assert_equal "Manchester United", find_best_match(england, "Man Utd")
    assert_equal "Newcastle United", find_best_match(england, "Newcastle")
    assert_equal "Norwich City", find_best_match(england, "Norwich")
    assert_equal "Queens Park Rangers", find_best_match(england, "QPR")
    assert_equal "Stoke City", find_best_match(england, "Stoke")
    assert_equal "Swansea City", find_best_match(england, "Swansea")
    assert_equal "Tottenham Hotspur", find_best_match(england, "Tottenham")
    assert_equal "West Bromwich Albion", find_best_match(england, "West Brom")
    assert_equal "Wigan Athletic", find_best_match(england, "Wigan")
    assert_equal "Wolverhampton", find_best_match(england, "Wolves")
  end
  
 private
  def find_best_match(set, element)
    set.map{|x| [ x, FuzzyTeamMatch.new.getDistance(ActiveSupport::Inflector.transliterate(x).downcase, ActiveSupport::Inflector.transliterate(element).downcase)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
  end
end
