class TeamGroupOddsHistory < ApplicationRecord
  serialize :odds

  belongs_to :team_group

  validates :recorded_on, presence: true
  validates :captured_at, presence: true
  validates :odds, presence: true
  validates :team_group_id, uniqueness: { scope: :recorded_on }
end
