"""Initial revision

Revision ID: 8cc733934278
Revises:
Create Date: 2025-01-14 13:09:27.719541

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = '8cc733934278'
down_revision: Union[str, None] = None
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table("protein",
            sa.Column("id",sa.Integer(),nullable=False),
            sa.Column("name",sa.String(length=30),nullable=False),
            sa.Column("pdb_id",sa.String(length=10),nullable=True),
            sa.PrimaryKeyConstraint("id")
    )
    op.create_table(
           "variant",
           sa.Column("id", sa.Integer(), nullable=False),
           sa.Column("chunk",sa.Integer(), nullable=False),
           sa.Column("pos", sa.Integer(), nullable=False),
           sa.Column("condition", sa.String(length=30), nullable=False),
           sa.Column("aa", sa.String(length=30), nullable=False),
           sa.Column("log2_fold_change", sa.Double(), nullable=False),
           sa.Column("log2_std_error", sa.Double(), nullable=False),
           sa.Column("statistic", sa.Double(), nullable=False),
           sa.Column("p_value", sa.Double(), nullable=False),
           sa.Column("version", sa.String(length=30), nullable=False),
           sa.Column("protein_id", sa.Integer(), nullable=False),
           sa.Column("created_on", sa.DateTime(), nullable=False),
           sa.PrimaryKeyConstraint("id"),
           sa.ForeignKeyConstraint(["protein_id"], ["protein.id"], ondelete="CASCADE"),
       )


def downgrade() -> None:
    op.drop_table("variant")
    op.drop_table("protein")
